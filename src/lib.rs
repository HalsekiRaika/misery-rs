use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use async_std::fs::{File, OpenOptions};
use async_std::io::{ReadExt, WriteExt};
use async_std::path::Path;
use async_std::sync::RwLock;
use async_std::task::block_on;
use once_cell::sync::OnceCell;

use serde::{Serialize, Deserialize};

fn get_default_cache_path() -> &'static str {
    static CACHE: OnceCell<String> = OnceCell::new();
    CACHE.get_or_init(|| {
        dotenv::var("CACHE_DEFAULT")
            .unwrap_or_else(|_| String::from("./.cache.json"))
    })
}

pub struct MiseryHandler<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        K: serde::de::DeserializeOwned + serde::Serialize,
        V: Clone + Hash + Eq + PartialEq,
        V: serde::de::DeserializeOwned + serde::Serialize
{
    path: String,
    caches: Arc<RwLock<HashSet<CacheWrapper<K, V>>>>
}

impl<K, V> MiseryHandler<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        K: serde::de::DeserializeOwned + serde::Serialize,
        V: Clone + Hash + Eq + PartialEq,
        V: serde::de::DeserializeOwned + serde::Serialize
{
    pub fn load_from_blocking<P>(path: P) -> MiseryHandler<K, V> where P: Into<String> + Clone {
        Self { path: path.clone().into(), caches: Arc::new(RwLock::new(serde_json::from_str(&block_on(Self::read(path.into()))).unwrap_or_default())) }
    }

    pub async fn abs(&self, cache: CacheWrapper<K, V>) {
        self.remove(cache.as_ref_key()).await;
        self.push(cache).await;
    }

    pub async fn push(&self, cache: CacheWrapper<K, V>) {
        self.caches.write().await.insert(cache);
    }

    pub async fn find(&self, key: &K) -> Option<CacheWrapper<K, V>> {
        self.caches.read().await.iter()
            .find(|temp| temp.as_ref_key() == key)
            .map(|cache| cache.to_owned())
    }

    pub async fn find_value(&self, key: &K) -> Option<V> {
        self.caches.read().await.iter()
            .find(|temp| temp.as_ref_key() == key)
            .map(|cache| cache.value())
    }

    pub async fn remove(&self, key: &K) {
        self.caches.write().await.retain(|cache| cache.as_ref_key() != key);
    }

    pub async fn all_items(&self) -> Vec<CacheWrapper<K, V>> {
        self.caches.read().await.iter().cloned().collect::<Vec<_>>()
    }

    async fn write(&self) {
        let mut file = Self::open(&self.path).await;
        file.set_len(0).await.expect("");
        let cache_string = serde_json::to_string(&self.caches.read().await.iter().collect::<Vec<_>>())
            .expect("cannot serialize to string");
        let _ = file.write(cache_string.as_ref()).await;
    }

    async fn read<P>(path: P) -> String where P: AsRef<Path> {
        let mut file = Self::open(path).await;
        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf).await
            .expect("read failed");
        buf
    }

    async fn open<P>(path: P) -> File where P: AsRef<Path> {
        let path = path.as_ref();
        OpenOptions::new()
            .read(true).write(true).open(path).await
            .unwrap_or_else(|_| block_on(
                OpenOptions::new().create(true)
                    .write(true).read(true).open(path))
                .expect("cannot open"))
    }
}

impl<K, V> Default for MiseryHandler<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        K: serde::de::DeserializeOwned + serde::Serialize,
        V: Clone + Hash + Eq + PartialEq,
        V: serde::de::DeserializeOwned + serde::Serialize
{
    fn default() -> Self {
        MiseryHandler::load_from_blocking(get_default_cache_path())
    }
}

impl<K, V> Drop for MiseryHandler<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        K: serde::de::DeserializeOwned + serde::Serialize,
        V: Clone + Hash + Eq + PartialEq,
        V: serde::de::DeserializeOwned + serde::Serialize
{
    fn drop(&mut self) {
        block_on(self.write());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct CacheWrapper<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        V: Clone + Hash + Eq + PartialEq,
{
    key: K,
    value: V,
}

impl<K, V> CacheWrapper<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        V: Clone + Hash + Eq + PartialEq,
{
    pub fn new(key: K, value: V) -> CacheWrapper<K, V> {
        Self { key, value }
    }

    pub fn as_ref_key(&self) -> &K {
        &self.key
    }

    pub fn as_ref_value(&self) -> &V {
        &self.value
    }

    pub fn key(&self) -> K {
        self.key.clone()
    }

    pub fn value(&self) -> V {
        self.value.clone()
    }

    pub fn rebase_key(mut self, rebase: K) -> CacheWrapper<K, V> {
        self.key = rebase;
        self
    }

    pub fn rebase_value(mut self, rebase: V) -> CacheWrapper<K, V> {
        self.value = rebase;
        self
    }
}

impl<K, V> AsRef<CacheWrapper<K, V>> for CacheWrapper<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        V: Clone + Hash + Eq + PartialEq,
{
    fn as_ref(&self) -> &CacheWrapper<K, V> {
        self
    }
}

impl<K, V> AsMut<CacheWrapper<K, V>> for CacheWrapper<K, V>
  where K: Clone + Hash + Eq + PartialEq,
        V: Clone + Hash + Eq + PartialEq,
{
    fn as_mut(&mut self) -> &mut CacheWrapper<K, V> {
        self
    }
}

#[cfg(test)]
mod test {
    use std::marker::PhantomData;
    use std::path::Path;
    use futures::StreamExt;
    use serde::{Serialize, Deserialize};
    use crate::{CacheWrapper, MiseryHandler};

    #[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
    #[serde(transparent)]
    pub struct StringId<T> {
        id: String,
        #[serde(skip)]
        _mark: PhantomData<T>
    }

    impl<T> StringId<T> {
        pub fn new<I>(id: I) -> StringId<T> where I: Into<String> {
            Self { id: id.into(), _mark: PhantomData }
        }
    }

    impl<T> From<String> for StringId<T> {
        fn from(s: String) -> Self {
            StringId::new(s)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
    pub struct HandlingData {
        id: StringId<HandlingData>,
        data_1: String,
        data_2: i32
    }

    impl HandlingData {
        fn new<I, S>(id: I, str_data: S, int_data: i32) -> HandlingData where I: Into<String>, S: Into<String> {
            Self { id: StringId::<Self>::new(id), data_1: str_data.into(), data_2: int_data }
        }
    }

    #[tokio::test]
    async fn usage_test() {
        {
            let external_cache = MiseryHandler::<StringId<HandlingData>, HandlingData>::load_from_blocking("./test/usage_test.json");

            let vec = vec![
                CacheWrapper::new(StringId::<HandlingData>::new("abc"), HandlingData::new("abc", "test_1", 123)),
                CacheWrapper::new(StringId::<HandlingData>::new("def"), HandlingData::new("def", "test_2", 456)),
                CacheWrapper::new(StringId::<HandlingData>::new("ghi"), HandlingData::new("ghi", "test_3", 789)),
                CacheWrapper::new(StringId::<HandlingData>::new("jkm"), HandlingData::new("jkm", "test_4", 321)),
                CacheWrapper::new(StringId::<HandlingData>::new("nop"), HandlingData::new("nop", "test_5", 654)),
            ];

            for cache in vec {
                let external_cache = &external_cache;
                external_cache.push(cache).await;
            }

            let find_test_1 = external_cache.find_value(&StringId::<HandlingData>::new("abc")).await;
            assert_eq!(find_test_1, Some(HandlingData::new("abc", "test_1", 123)));

            external_cache.remove(&StringId::<HandlingData>::new("def")).await;
            let removed_test_2 = external_cache.find_value(&StringId::<HandlingData>::new("def")).await;
            assert_eq!(removed_test_2, None);

            let mut overwrite_test_3 = external_cache.find(&StringId::<HandlingData>::new("ghi")).await;
            let overwrite_test_3 = overwrite_test_3.unwrap()
                .rebase_value(HandlingData::new("ghi", "test_3_overwrite", 777));
            external_cache.remove(&StringId::<HandlingData>::new("ghi")).await;
            external_cache.push(overwrite_test_3.to_owned()).await;
            let test_3 = external_cache.find_value(&StringId::<HandlingData>::new("ghi")).await;
            assert_eq!(test_3, Some(HandlingData::new("ghi", "test_3_overwrite", 777)));
        }
        println!("cache handler dropped.");
        assert!(Path::new("./test/usage_test.json").exists())
    }

    #[tokio::test]
    async fn thread_safe_test() {
        {
            let vec = vec![
                CacheWrapper::new(StringId::<HandlingData>::new("abc"), HandlingData::new("abc", "test_1", 123)),
                CacheWrapper::new(StringId::<HandlingData>::new("def"), HandlingData::new("def", "test_2", 456)),
                CacheWrapper::new(StringId::<HandlingData>::new("ghi"), HandlingData::new("ghi", "test_3", 789)),
                CacheWrapper::new(StringId::<HandlingData>::new("jkm"), HandlingData::new("jkm", "test_4", 321)),
                CacheWrapper::new(StringId::<HandlingData>::new("nop"), HandlingData::new("nop", "test_5", 654)),
                CacheWrapper::new(StringId::<HandlingData>::new("qrs"), HandlingData::new("qrs", "test_6", 987)),
            ];

            let handler = MiseryHandler::<StringId<HandlingData>, HandlingData>::load_from_blocking("./test/thread_safe_test.json");


            futures::stream::iter(vec.iter()).map(|cache| {
                let handler = &handler;
                async move {
                    handler.push(cache.to_owned()).await;
                    cache
                }
            }).buffer_unordered(4)
                .collect::<Vec<_>>()
                .await;
        }
        println!("cache handler dropped.");
        assert!(Path::new("./test/thread_safe_test.json").exists())
    }

    #[tokio::test]
    async fn all_method_test() {
        {
            let vec = vec![
                CacheWrapper::new(StringId::<HandlingData>::new("abc"), HandlingData::new("abc", "test_1", 123)),
                CacheWrapper::new(StringId::<HandlingData>::new("def"), HandlingData::new("def", "test_2", 456)),
                CacheWrapper::new(StringId::<HandlingData>::new("ghi"), HandlingData::new("ghi", "test_3", 789)),
                CacheWrapper::new(StringId::<HandlingData>::new("jkm"), HandlingData::new("jkm", "test_4", 321)),
                CacheWrapper::new(StringId::<HandlingData>::new("nop"), HandlingData::new("nop", "test_5", 654)),
                CacheWrapper::new(StringId::<HandlingData>::new("qrs"), HandlingData::new("qrs", "test_6", 987)),
            ];

            let handler = MiseryHandler::<StringId<HandlingData>, HandlingData>::load_from_blocking("./test/all_method_test.json");


            futures::stream::iter(vec.iter()).map(|cache| {
                let handler = &handler;
                async move {
                    handler.push(cache.to_owned()).await;
                    cache
                }
            }).buffer_unordered(4)
                .collect::<Vec<_>>()
                .await;
        }
        println!("cache handler dropped.");
        assert!(Path::new("./test/thread_safe_test.json").exists());

        {
            let handler = MiseryHandler::<StringId<HandlingData>, HandlingData>::load_from_blocking("./test/all_method_test.json");
            handler.all().await.iter().for_each(|item| println!("{:?}", item.as_ref_key()));
        }
    }
}