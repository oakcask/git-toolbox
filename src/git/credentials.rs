use std::{fs::File, path::PathBuf};

use git2::Cred;
use log::info;

#[derive(Default)]
struct SshState {
    id_candidates: Vec<&'static str>,
}

enum Method {
    Unavailable,
    SshAgent,
    SshId(SshState),
    Helper,
    Default,
}

pub struct CredentialCallback {
    config: git2::Config,
    next_method: Option<Method>,
}

impl CredentialCallback {
    pub fn new(config: git2::Config) -> Self {
        CredentialCallback {
            config,
            next_method: None,
        }
    }

    pub fn try_next(
        &mut self,
        url: &str,
        username: Option<&str>,
        allowed_types: git2::CredentialType,
    ) -> Result<Cred, git2::Error> {
        match self.next_method.as_mut() {
            Some(Method::Helper) => {
                self.next_method = Some(Method::Unavailable); // never retry if the helper fails.
                info!("trying credential helper");
                Cred::credential_helper(&self.config, url, username)
            }
            Some(Method::SshAgent) => {
                self.next_method = Some(Method::SshId(SshState {
                    id_candidates: vec!["id_ecdsa", "id_ecdsa-sk", "id_ed25519", "id_ed25519-sk"],
                }));
                Cred::ssh_key_from_agent(username.unwrap_or("git"))
            }
            Some(Method::SshId(state)) => {
                if let Some(key) = state.id_candidates.pop() {
                    let mut path = PathBuf::from(std::env::var("HOME").unwrap());
                    path.push(".ssh");
                    path.push(key);
                    let path = path.as_path();
                    info!("trying ssh key {path:?}");
                    if let Err(_) = File::open(path) {
                        self.try_next(url, username, allowed_types)
                    } else {
                        Cred::ssh_key(username.unwrap_or("git"), None, path, None)
                    }
                } else {
                    Err(git2::Error::from_str("no valid credentials available"))
                }
            }
            Some(Method::Default) => {
                self.next_method = Some(Method::Unavailable); // never retry if the default fails.
                info!("trying default credential");
                Cred::default()
            }
            Some(Method::Unavailable) => {
                Err(git2::Error::from_str("no valid credentials available"))
            }
            None => {
                self.next_method = Some(Self::choose_method(allowed_types));
                self.try_next(url, username, allowed_types)
            }
        }
    }

    fn choose_method(allowed_types: git2::CredentialType) -> Method {
        if allowed_types.is_user_pass_plaintext() {
            Method::Helper
        } else if allowed_types.is_default() {
            Method::Default
        } else if allowed_types.is_ssh_key() {
            Method::SshAgent
        } else {
            Method::Unavailable
        }
    }
}
