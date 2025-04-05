use std::sync::{Arc, Weak, Mutex};


// 全局注册中心
lazy_static::lazy_static! {
    static ref REGISTRY: Mutex<Vec<Weak<dyn Sampler>>> = Mutex::new(Vec::new());
}

trait Sampler: Send + Sync {
    fn sample(&self);
}

struct MySampler {
    weak_self: Mutex<Option<Weak<Self>>>, // 存储自身的弱引用
    data: u32,
}

impl MySampler {
    // 关键：使用 new_cyclic 在构造期间获取弱引用
    pub fn new(data: u32) -> Arc<Self> {
        Arc::new_cyclic(|weak| {
            let this = Self {
                weak_self: Mutex::new(Some(weak.clone())), // ✅ 正确获取弱引用
                data,
            };
            this.after_init();
            this
        })
    }

    // 初始化后操作
    fn after_init(&self) {
        let guard = self.weak_self.lock().unwrap();
        let weak = guard.as_ref().unwrap().clone();
        REGISTRY.lock().unwrap().push(weak); // 安全注册
    }
}

impl Sampler for MySampler {
    fn sample(&self) {
        println!("Sampling data: {}", self.data);
    }
}


struct FlawedSampler {
    weak_self: Mutex<Option<Weak<Self>>>,
    data: u32,
}

impl FlawedSampler {
    // ❌ 错误方式：尝试在构造后设置 weak_self
    pub fn new(data: u32) -> Arc<Self> {
        let arc = Arc::new(Self {
            weak_self: Mutex::new(None), // 初始化为空
            data,
        });
        
        // 尝试回填弱引用
        let weak = Arc::downgrade(&arc);
        *arc.weak_self.lock().unwrap() = Some(weak.clone());
        

        println!("weak : {}", weak.upgrade().is_some());
        // 立即注册会失败！
        REGISTRY.lock().unwrap().push(weak); // 😧 此时 weak 可能尚未完全初始化
        arc
    }
}

impl Sampler for FlawedSampler {
    fn sample(&self) {
        println!("Sampling data: {}", self.data);
    }
}


fn main () {
    {
        let sampler = MySampler::new(42);
        let registry = REGISTRY.lock().unwrap();
    
        assert_eq!(registry.len(), 1);
        assert!((*registry)[0].upgrade().is_some()); // 弱引用有效
    
    }

    {
        let sampler = FlawedSampler::new(42);
        let registry = REGISTRY.lock().unwrap();
    
        assert_eq!(registry.len(), 2);
        let weak = &registry;
        assert!((*weak)[1].upgrade().is_some());
        // *weak , 对MutexGuard<Vec<Weak<_>>> 的解引用，获取Vec
        (*weak)[1].upgrade().unwrap().sample(); // 可能 panic
    }

}
