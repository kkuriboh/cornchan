use serde::{Deserialize, Serialize};

use crate::traits::Id;

#[derive(Serialize, Deserialize)]
pub struct Board {
    name: String,
    slug: String,
    description: String,
}

#[allow(unused)]
pub fn test_board() -> Board {
    Board {
        name: "test board".into(),
        slug: "test_board".into(),
        description: "board for testing shit".into(),
    }
}

impl Id for Board {
    fn ident(&self) -> String {
        format!("board:{}", self.slug)
    }
}
