// Copyright 2025 KenForever1
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! 实现数据的组合器，用于线程本地数据的合并

use std::marker::PhantomData;
use std::cell::UnsafeCell;
use thread_local::ThreadLocal;
use parking_lot::Mutex;

/// 提供标准组合操作，如总和、最大值、最小值和平均值
pub trait Combiner<T>: Send + Sync + Clone {
    /// 组合两个值，返回组合后的结果
    fn combine(&self, v1: T, v2: T) -> T;
    
    /// 对值进行修改，执行某种计算操作
    fn modify(&self, v: T) -> T;
    
    /// 获取组合器的名称
    fn name(&self) -> &str;
}


/// 一个线程本地的Agent
pub struct Agent<T> {
    /// 存储的值
    pub value: T,
    /// Agent的ID
    pub id: u64,
}

/// 用于帮助组合多个线程的数据
pub struct AgentCombiner<T, Op> 
where
    T: Send + Sync,
    Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    /// 线程本地存储
    tls: ThreadLocal<Mutex<Agent<T>>>,
    /// 默认值
    identity: T,
    /// 组合操作
    op: Op,
    /// 下一个Agent的ID
    next_id: u64,
    /// 变量名称
    name: UnsafeCell<String>,
} 

unsafe impl<T, Op> Send for AgentCombiner<T, Op> 
where
    T: Send + Sync,
    Op:  Combiner<T> + Send + Sync + 'static + Clone,
{}
unsafe impl<T, Op> Sync for AgentCombiner<T, Op> 
where
    T: Send + Sync,
    Op:  Combiner<T> + Send + Sync + 'static + Clone,
{}

impl<T, Op> AgentCombiner<T, Op>
where
    T: Clone + Send + Sync + 'static,
    Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    /// 创建新的组合器
    pub fn new(identity: T, op: Op, name: String) -> Self {
        Self {
            tls: ThreadLocal::new(),
            identity,
            op,
            next_id: 1,
            name: UnsafeCell::new(name),
        }
    }
    
    /// 获取或创建当前线程的Agent
    pub fn get_or_create_tls_agent(&mut self) -> Option<&Mutex<Agent<T>>> {
        Some(self.tls.get_or(|| {
            let id = self.next_id;
            self.next_id += 1;
            
            Mutex::new(Agent {
                value: self.identity.clone(),
                id,
            })
        }))
    }
    
    /// 对所有Agent的值执行组合操作
    pub fn combine_agents(&self) -> T {
        let result = self.identity.clone();
        
        for agent in self.tls.iter() {
            let agent_value = agent.lock().value.clone();
            self.op.combine(result.clone(), agent_value);
        }
        
        result
    }
    
    /// 重置所有Agent的值，并返回组合前的值
    pub fn reset_all_agents(&self) -> T {
        let result = self.combine_agents();
        
        for agent in self.tls.iter() {
            let mut guard = agent.lock();
            guard.value = self.identity.clone();
        }
        
        result
    }
    
    /// 获取线程本地存储迭代器
    pub fn iter(&self) -> thread_local::Iter<Mutex<Agent<T>>> {
        self.tls.iter()
    }
    
    /// 获取所有线程的Agent数量
    pub fn agent_count(&self) -> usize {
        let mut count = 0;
        for _ in self.tls.iter() {
            count += 1;
        }
        count
    }
    
    /// 获取组合操作
    pub fn op(&self) -> &Op {
        &self.op
    }

        
    /// 设置变量名称
    pub fn set_name(&self, name: String) {
        unsafe {
            *self.name.get() = name;
        }
    }
    
    /// 获取变量名称
    pub fn name(&self) -> &str {
        unsafe {
            &*self.name.get()
        }
    }
}

/// 用于帮助修改Agent中的值
pub struct AgentModifier<T, V, F> {
    _phantom: PhantomData<(T, V)>,
    modifier: F,
}

impl<T, V, F> AgentModifier<T, V, F>
where
    F: Fn(&mut T, &V),
{
    /// 创建新的修改器
    pub fn new(modifier: F) -> Self {
        Self {
            _phantom: PhantomData,
            modifier,
        }
    }
    
}

/// 将组合操作包装为修改器
pub struct OpAsModifier<T, Op>(pub Op, pub PhantomData<T>);

impl<T, Op> OpAsModifier<T, Op>
where
    Op: Combiner<T>,
{
    /// 创建新的包装器
    pub fn new(op: Op) -> Self {
        Self(op, PhantomData)
    }
}

// 新增统一的修改器 trait
pub trait Modifier<T, V> {
    fn modify(&self, value: &mut T, arg: &V);
}

// 为 AgentModifier 实现 Modifier
impl<T, V, F> Modifier<T, V> for AgentModifier<T, V, F>
where
    F: Fn(&mut T, &V) + Send + Sync,
{
    fn modify(&self, value: &mut T, arg: &V) {
        (self.modifier)(value, arg);
    }
}

// 修改 OpAsModifier 的实现方式
impl<T, Op> Modifier<T, T> for OpAsModifier<T, Op>
where
    Op: Combiner<T> + Clone,
    T: Clone, // 需要克隆能力来适配 Combiner 的传值语义
{
    fn modify(&self, value: &mut T, arg: &T) {
        let current = value.clone();
        *value = self.0.combine(current, arg.clone());
    }
}

// 组合器到修改器的转换实现
impl<T, Op> From<Op> for OpAsModifier<T, Op>
where
    Op: Combiner<T> + Clone,
{
    fn from(op: Op) -> Self {
        OpAsModifier(op, PhantomData)
    }
}


/// 分发错误的Handler
pub struct IgnoreErrorHandler;

/// 处理样本收集错误接口
pub trait SampleErrorHandler : Send + Sync {
    /// 处理错误
    fn on_error(&self, error: &str);
}

impl SampleErrorHandler for IgnoreErrorHandler {
    fn on_error(&self, _error: &str) {
        // 忽略错误
    }
}

/// 记录错误的Handler
pub struct LoggingErrorHandler;

impl SampleErrorHandler for LoggingErrorHandler {
    fn on_error(&self, error: &str) {
        log::error!("Sampler error: {}", error);
    }
} 