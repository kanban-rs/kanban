use crate::KanbanResult;
use async_trait::async_trait;

#[async_trait]
pub trait Repository<T, Id> {
    async fn find_by_id(&self, id: Id) -> KanbanResult<Option<T>>;
    async fn find_all(&self) -> KanbanResult<Vec<T>>;
    async fn save(&self, entity: &T) -> KanbanResult<T>;
    async fn delete(&self, id: Id) -> KanbanResult<()>;
}

#[async_trait]
pub trait Service<T, Id> {
    async fn get(&self, id: Id) -> KanbanResult<T>;
    async fn list(&self) -> KanbanResult<Vec<T>>;
    async fn create(&self, entity: T) -> KanbanResult<T>;
    async fn update(&self, id: Id, entity: T) -> KanbanResult<T>;
    async fn delete(&self, id: Id) -> KanbanResult<()>;
}
