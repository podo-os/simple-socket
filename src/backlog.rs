pub struct Backlog(i32);

impl Default for Backlog {
    fn default() -> Self {
        Self(0)
    }
}

impl From<Backlog> for i32 {
    fn from(backlog: Backlog) -> Self {
        backlog.0
    }
}

impl From<i32> for Backlog {
    fn from(backlog: i32) -> Self {
        Self(backlog)
    }
}
