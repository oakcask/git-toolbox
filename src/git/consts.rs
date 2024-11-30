/// git_index_stage_t
pub enum IndexStage {
    Any,
    Normal,
    Anscestor,
    Ours,
    Theirs,
}

impl From<IndexStage> for i32 {
    fn from(val: IndexStage) -> Self {
        match val {
            IndexStage::Any => -1,
            IndexStage::Normal => 0,
            IndexStage::Anscestor => 1,
            IndexStage::Ours => 2,
            IndexStage::Theirs => 3,
        }
    }
}
