use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Hash)]
pub struct Output {
    pub count: i32,
    pub config: String,
    pub a: String,
}
