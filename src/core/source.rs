pub trait Source {
    async fn connect(&mut self);
    async fn poll(&mut self);
}
