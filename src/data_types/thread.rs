use serde::{Deserialize, Serialize};

use crate::traits::Id;

#[derive(Serialize, Deserialize, Debug)]
pub struct ThreadPayload {
    pub id: u64,
    pub nickname: String,
    pub title: String,
    pub content: String,
    pub timestamp: u64,
    pub board: String,
    pub image_1: Option<String>,
    pub image_2: Option<String>,
    pub image_3: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Thread {
    Parent(ThreadPayload),
    Comment {
        parent_thread: u64,
        #[serde(flatten)]
        payload: ThreadPayload,
    },
}

impl Id for Thread {
    fn ident(&self) -> String {
        match self {
            Thread::Parent(ThreadPayload {
                id,
                board,
                timestamp,
                ..
            }) => format!("thread:{board}:{timestamp}:{id}",),
            Thread::Comment {
                parent_thread,
                payload:
                    ThreadPayload {
                        id,
                        board,
                        timestamp,
                        ..
                    },
            } => format!("thread:{board}:{timestamp}:{id}:{parent_thread}",),
        }
    }
}
