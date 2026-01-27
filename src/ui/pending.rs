use std::time::Duration;

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Pending<T> {
    pub request_id: Uuid,
    pub data: T,
    delay: f32,
}

impl<T> Pending<T> {
    pub fn new(data: T) -> Self {
        Pending {
            request_id: Uuid::new_v4(),
            data,
            delay: 0.,
        }
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }

    pub async fn map<U, F>(self, f: F) -> Pending<U>
    where
        F: AsyncFnOnce(T) -> U,
    {
        let data = f(self.data).await;
        std::thread::sleep(Duration::from_secs_f32(self.delay));
        Pending {
            request_id: self.request_id,
            data,
            delay: self.delay,
        }
    }
}

impl<T> From<(Uuid, T)> for Pending<T> {
    fn from((request_id, data): (Uuid, T)) -> Self {
        Pending {
            request_id,
            data,
            delay: 0.,
        }
    }
}
