# misery-rs &emsp; [![misery: rustc 1.60+]][Rust 1.60] 

[misery: rustc 1.60+]: https://img.shields.io/badge/misery-rustc1.60%2B-ff69b4?style=flat-square&logo=appveyor.svg
[Rust 1.60]: https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html
##### - About library naming...  
##### In creating my first "proper" working library, I feel like "misery" that I wanted to write better...  

---

## Usage
```toml
[dependencies]
misery-rs = { git = "https://github.com/ReiRokusanami0010/misery-rs" }
```

```rust
use serde::{Serialize, Deserialize};

async fn asynchronous_handling() {
    {
        /// External files generated for caching are generated (or saved) at the time of Drop.
        /// Note: this process uses blocking and is a synchronous process.
        let caching: MiseryHandler<StringId<Article>, Article> = MiseryHandler::load_from_blocking("./test/article_cache.json");

        let vec = vec![
            CacheWrapper::new(StringId::<Article>::new("abc"), Article::new("abc", "test_1", 123)),
            CacheWrapper::new(StringId::<Article>::new("def"), Article::new("def", "test_2", 456)),
            CacheWrapper::new(StringId::<Article>::new("ghi"), Article::new("ghi", "test_3", 789)),
            CacheWrapper::new(StringId::<Article>::new("jkm"), Article::new("jkm", "test_4", 321)),
            CacheWrapper::new(StringId::<Article>::new("nop"), Article::new("nop", "test_5", 654)),
        ];

        for item in vec {
            let caching = &caching;
            caching.push(item).await;
        }
    }
    assert!(std::path::Path::new("./test/usage_test.json").exists());
}

/// `Clone`, `Hash`, `Eq`, `PartialEq`, and `PartialEq` required by misery-rs for caching.
/// Also, `serde::Serialize` and `serde::deserialize` must be implemented.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct StringId<T> {
    id: String,
    #[serde(skip)]
    _mark: std::marker::PhantomData<T>
}

impl<T> StringId<T> {
    fn new<I>(id: I) -> StringId<T> where I: Into<String> {
        Self { id: id.into(), _mark: std::marker::PhantomData }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Article {
    id: StringId<Article>,
    title: String,
    page: i32
}

impl Article {
    fn new<I, S>(id: I, title: S, page: i32) -> Article
        where I: Into<String>, S: Into<String> 
    {
        Self { 
            id: StringId::<Self>::new(id),
            title: title.into(),
            page
        }
    }
}
```