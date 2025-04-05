use std::sync::{Arc, Mutex};

/// Mutex 成员变量的结构体实现 Clone 

#[derive(Debug)]
struct ThreadSafeData {
    counter: Mutex<i32>,      // 被互斥锁保护的计数器
    data: Mutex<Vec<String>>, // 被互斥锁保护的复杂数据
}

impl ThreadSafeData {
    pub fn new(init_val: i32) -> Self {
        Self {
            counter: Mutex::new(init_val),
            data: Mutex::new(vec!["default".to_string()]),
        }
    }
}

// 手动实现 Clone（无法通过 #[derive] 自动实现）
impl Clone for ThreadSafeData {
    fn clone(&self) -> Self {
        // 安全获取锁并克隆数据
        let counter_val = *self.counter.lock().unwrap(); // 解引用获取 i32
        let data_clone = self.data.lock().unwrap().clone(); // 克隆 Vec<String>
        
        ThreadSafeData {
            counter: Mutex::new(counter_val), // 创建新 Mutex
            data: Mutex::new(data_clone),     // 创建新 Mutex
        }
    }
}


fn test_clone() {
    let original = ThreadSafeData::new(5);
    original.data.lock().unwrap().push("test".to_string());

    let cloned = original.clone();
    
    // 验证计数器克隆
    assert_eq!(*cloned.counter.lock().unwrap(), 5);
    
    // 修改原数据验证独立性
    *original.counter.lock().unwrap() += 1;
    original.data.lock().unwrap().push("modified".to_string());
    
    // 验证克隆体数据独立
    assert_eq!(*cloned.counter.lock().unwrap(), 5); // 原值保持
    assert_eq!(cloned.data.lock().unwrap().len(), 2); // 原始克隆时的数据
}

// #[derive(Clone)] // ❌ 会编译失败
// struct BadClone {
//     mutex_data: Mutex<String>
// }

// Mutex 本身没有实现 Clone trait
// 自动派生要求所有字段都实现 Clone



// 当需要跨线程共享克隆体时，可以结合 Arc 使用

#[derive(Clone)]
struct ArcData {
    shared: Arc<Mutex<Vec<u8>>>, // Arc 可克隆
}

impl ArcData {
    // 克隆时共享同一个 Mutex
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(vec![])),
        }
    }
}

fn test_share(){
    // 此时克隆体会共享同一个 Mutex
    let a = ArcData::new();
    let b = a.clone();
    a.shared.lock().unwrap().push(1);
    assert_eq!(b.shared.lock().unwrap().len(), 1); // 共享修改
}

fn main() {
    test_clone();

    test_share();
}