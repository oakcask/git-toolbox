# Changelog

## [2.10.1](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.10.0...git-toolbox-v2.10.1) (2026-04-15)


### Bug Fixes

* **git-stale:** do authentication before push ([#431](https://github.com/oakcask/git-toolbox/issues/431)) ([7242b67](https://github.com/oakcask/git-toolbox/commit/7242b67393b9f8506743cba0091de43a5b114f9e))

## [2.10.0](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.7...git-toolbox-v2.10.0) (2026-04-14)


### Features

* **git-stale:** add `--remote` to operate tracking branches ([#425](https://github.com/oakcask/git-toolbox/issues/425)) ([55febce](https://github.com/oakcask/git-toolbox/commit/55febce97baca87a527927b1eda43b86df224fd7))
* **git-stale:** honor protected branch setting as well as git-dah ([#421](https://github.com/oakcask/git-toolbox/issues/421)) ([8c93a87](https://github.com/oakcask/git-toolbox/commit/8c93a87d75045c0a278ec18b4bac4a7773dda4cb))


### Bug Fixes

* **git-stale:** tighten to fully qualified destination refspec ([#426](https://github.com/oakcask/git-toolbox/issues/426)) ([f55b4cd](https://github.com/oakcask/git-toolbox/commit/f55b4cd0ebb4781cc9eba56ae3f1b6f6b2eb5a71))

## [2.9.7](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.6...git-toolbox-v2.9.7) (2026-04-11)


### Performance Improvements

* **git-whose:** reduce use of iter and allocation ([#414](https://github.com/oakcask/git-toolbox/issues/414)) ([177c129](https://github.com/oakcask/git-toolbox/commit/177c129f11106f4e59e157e2e821f42843bebde5))

## [2.9.6](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.5...git-toolbox-v2.9.6) (2026-04-10)


### Bug Fixes

* **git-dah:** check only the upstream remote ([#412](https://github.com/oakcask/git-toolbox/issues/412)) ([9560d23](https://github.com/oakcask/git-toolbox/commit/9560d23b94567155b449f5349a6271f8bcde7b63))
* **git-stale:** prevent mass deletion by typo ([#410](https://github.com/oakcask/git-toolbox/issues/410)) ([b66177a](https://github.com/oakcask/git-toolbox/commit/b66177a3e8c871b51cacb8c66d2734b85ec194b4))

## [2.9.5](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.4...git-toolbox-v2.9.5) (2026-04-09)


### Bug Fixes

* release binaries (release workflow fixed) ([#406](https://github.com/oakcask/git-toolbox/issues/406)) ([b5d640b](https://github.com/oakcask/git-toolbox/commit/b5d640b587b019beec42e30fc20d89ba81b90048))

## [2.9.4](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.3...git-toolbox-v2.9.4) (2026-04-09)


### Bug Fixes

* release binaries (release workflow fixed) ([#403](https://github.com/oakcask/git-toolbox/issues/403)) ([bc8842f](https://github.com/oakcask/git-toolbox/commit/bc8842ffe9ed0f071a29536fb30ed398c8239bfa))

## [2.9.3](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.2...git-toolbox-v2.9.3) (2026-04-09)


### Bug Fixes

* **git-whose:** actual bare repo support ([#401](https://github.com/oakcask/git-toolbox/issues/401)) ([02772b4](https://github.com/oakcask/git-toolbox/commit/02772b4ed7d3ca383b6e19ec45db26830bae346c))

## [2.9.2](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.1...git-toolbox-v2.9.2) (2026-04-06)


### Bug Fixes

* **deps:** update rust crate chrono to 0.4.44 ([#379](https://github.com/oakcask/git-toolbox/issues/379)) ([4289016](https://github.com/oakcask/git-toolbox/commit/428901662014bfde3bbd451b4d1f56d67ddeda0c))
* **deps:** update rust crate clap to 4.6.0 ([#390](https://github.com/oakcask/git-toolbox/issues/390)) ([888ce79](https://github.com/oakcask/git-toolbox/commit/888ce79b0b4570079b183d2c08496277e7713c1b))
* **deps:** update rust crate env_logger to 0.11.10 ([#381](https://github.com/oakcask/git-toolbox/issues/381)) ([0438190](https://github.com/oakcask/git-toolbox/commit/04381909feab737761ac5c051d5f956371f8d7fd))
* **deps:** update rust crate git2 to 0.20.4 ([#382](https://github.com/oakcask/git-toolbox/issues/382)) ([c8e5239](https://github.com/oakcask/git-toolbox/commit/c8e5239a922a7a5d05f6ad1b267d219d36e8a111))
* **deps:** update rust crate once_cell to 1.21.4 ([#383](https://github.com/oakcask/git-toolbox/issues/383)) ([95cee89](https://github.com/oakcask/git-toolbox/commit/95cee8991d0c6775c390dff8ef9be095a81298cf))
* **deps:** update rust crate regex to 1.12.3 ([#384](https://github.com/oakcask/git-toolbox/issues/384)) ([fcfea69](https://github.com/oakcask/git-toolbox/commit/fcfea69e2f6304fb6c4fdc69ee4dfc200a4a5f14))

## [2.9.1](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.9.0...git-toolbox-v2.9.1) (2026-03-25)


### Bug Fixes

* **deps:** update rust crate chrono to 0.4.43 ([#360](https://github.com/oakcask/git-toolbox/issues/360)) ([e4ec52b](https://github.com/oakcask/git-toolbox/commit/e4ec52bb7564d0cf2a86698899c3ceabc39f007e))
* **deps:** update rust crate clap to 4.5.54 ([#349](https://github.com/oakcask/git-toolbox/issues/349)) ([412104c](https://github.com/oakcask/git-toolbox/commit/412104c3915e7bc5aafdb4973804483ff4f9ec7c))
* **deps:** update rust crate clap to 4.5.55 ([#369](https://github.com/oakcask/git-toolbox/issues/369)) ([029219b](https://github.com/oakcask/git-toolbox/commit/029219b8b38cbbe6dc16077a0e1466e461d16cf3))
* **deps:** update rust crate clap to 4.5.56 ([#372](https://github.com/oakcask/git-toolbox/issues/372)) ([a14e6a7](https://github.com/oakcask/git-toolbox/commit/a14e6a768c0377c95edc74f5ac3073093a0f3d13))
* **deps:** update rust crate thiserror to 2.0.18 ([#364](https://github.com/oakcask/git-toolbox/issues/364)) ([bdbe5cb](https://github.com/oakcask/git-toolbox/commit/bdbe5cb680c81ee53c4889b8ed62217c95c942bf))
* **git-dah:** sanitize "."  ([#375](https://github.com/oakcask/git-toolbox/issues/375)) ([344083d](https://github.com/oakcask/git-toolbox/commit/344083dc916dfc12d59a97f084a1739c30a15125))

## [2.9.0](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.8.1...git-toolbox-v2.9.0) (2025-12-08)


### Features

* **git-dah:** add `--only-staged` option ([#327](https://github.com/oakcask/git-toolbox/issues/327)) ([910579d](https://github.com/oakcask/git-toolbox/commit/910579d11f1031917a3b125fec877cd65b5682ad))


### Bug Fixes

* **deps:** update rust crate git2 to 0.20.3 ([#324](https://github.com/oakcask/git-toolbox/issues/324)) ([1d1bd37](https://github.com/oakcask/git-toolbox/commit/1d1bd37874330482c59d58d0f7fbbf86fcb85d17))

## [2.8.1](https://github.com/oakcask/git-toolbox/compare/git-toolbox-v2.8.0...git-toolbox-v2.8.1) (2025-12-04)


### Bug Fixes

* **deps:** update rust crate clap to 4.5.49 ([#292](https://github.com/oakcask/git-toolbox/issues/292)) ([eda4178](https://github.com/oakcask/git-toolbox/commit/eda417877e239ec69d113091e2b33c1ae59f766b))
* **deps:** update rust crate clap to 4.5.50 ([#295](https://github.com/oakcask/git-toolbox/issues/295)) ([63cf650](https://github.com/oakcask/git-toolbox/commit/63cf65025290987e08253385373351f9a076b637))
* **deps:** update rust crate clap to 4.5.51 ([#298](https://github.com/oakcask/git-toolbox/issues/298)) ([5df36c7](https://github.com/oakcask/git-toolbox/commit/5df36c7310d4e97e84c69e31886a9b732d6f182e))
* **deps:** update rust crate clap to 4.5.52 ([#307](https://github.com/oakcask/git-toolbox/issues/307)) ([0e8634f](https://github.com/oakcask/git-toolbox/commit/0e8634fce4cef81801e099bb0699a6a5880b084e))
* **deps:** update rust crate clap to 4.5.53 ([#308](https://github.com/oakcask/git-toolbox/issues/308)) ([5a60913](https://github.com/oakcask/git-toolbox/commit/5a609139a3cd07ef322f4ff4c1c7a0b9dc03b31e))
* **deps:** update rust crate log to 0.4.29 ([#313](https://github.com/oakcask/git-toolbox/issues/313)) ([e63e262](https://github.com/oakcask/git-toolbox/commit/e63e262b14e82dd2100b024fe31e79199d0320d5))
* **deps:** update rust crate regex to 1.12.1 ([#291](https://github.com/oakcask/git-toolbox/issues/291)) ([a145113](https://github.com/oakcask/git-toolbox/commit/a145113fe07d4c145c6521aa4d51ea31651b7a12))
* **deps:** update rust crate regex to 1.12.2 ([#293](https://github.com/oakcask/git-toolbox/issues/293)) ([21b12e1](https://github.com/oakcask/git-toolbox/commit/21b12e15e042ee322493b8494a8c68f94aac4e73))
