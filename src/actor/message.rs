use serde::{Deserialize, Serialize};

pub use podo_core_driver::Message;

#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum Response<M> {
    Awk,
    Custom(M),
}

impl<M> Response<M> {
    pub fn unwrap_custom(self) -> M {
        match self {
            Self::Awk => unreachable!(),
            Self::Custom(m) => m,
        }
    }
}
