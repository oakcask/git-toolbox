/// git_index_stage_t
pub enum IndexStage {
    Any,
    Normal,
    Anscestor,
    Ours,
    Theirs,
}

impl Into<i32> for IndexStage {
    fn into(self) -> i32 {
        match self {
            Self::Any => -1,
            Self::Normal => 0,
            Self::Anscestor => 1,
            Self::Ours => 2,
            Self::Theirs => 3
        }
    }
}