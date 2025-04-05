// Copyright 2023 The Bvar-rust Authors
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

//! 实现用于将多个值规约为一个值的操作，如求和、求最大值等

use std::fmt;
use crate::variable::Variable;
use crate::detail::combiner::AgentCombiner;
use crate::detail::combiner::Combiner;
use std::fmt::Write;

/// 表示一个无效的反向操作
#[derive(Clone)]
pub struct VoidOp;

use std::sync::Arc;
use parking_lot::Mutex;


pub trait ReducerTrait<T, Op> {
    fn get_value(&self) -> T;

    fn reset(&self) -> T;

    fn op(&self) -> Op;
}

/// 提供将多值规约为单值的功能
///
/// Reducer使用`Op`将多个值规约为一个值: e1 Op e2 Op e3 ...
/// `Op`需要满足以下条件:
///   - 结合性:     a Op (b Op c) == (a Op b) Op c
///   - 交换性:     a Op b == b Op a;
///   - 无副作用:   a Op b在a和b固定时永远产生相同结果
#[derive(Clone)]
pub struct Reducer<T, Op> where
T: Clone + Send + Sync,
Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    /// 内部组合器
    combiner: Arc<Mutex<AgentCombiner<T, Op>>>,
    /// 最后一次暴露的名称
    _name: String,
}

impl<T, Op> Reducer<T, Op>
where
    T: Clone + Send + Sync + fmt::Display + 'static,
    Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    /// 创建新的Reducer
    pub fn new(identity: T, op: Op, name: String) -> Self {
        Self {
            combiner: Arc::new(Mutex::new(AgentCombiner::new(identity, op, name))),
            _name: String::new(),
        }
    }
    
    /// 添加一个值
    pub fn add(&mut self, value: T) -> &Self {
        let op = self.combiner.lock().op().clone();
        if let Some(agent) = self.combiner.lock().get_or_create_tls_agent() {
            let guard = agent.lock();
            op.combine(guard.value.clone(), value);
        }
        self
    }
    
    /// 获取规约后的值
    pub fn get_value(&self) -> T {
        self.combiner.lock().combine_agents()
    }
    
    /// 重置规约的值为identity
    pub fn reset(&self) -> T {
        self.combiner.lock().reset_all_agents()
    }
    
    /// 获取操作符实例
    pub fn op(&self) -> Op {
        self.combiner.lock().op().clone()
    }

}

impl<T, Op> ReducerTrait<T, Op> for Reducer<T, Op>
where
    T: Clone + Send + Sync + fmt::Display + 'static,
    Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    fn get_value(&self) -> T {
        self.get_value()
    }

    fn reset(&self) -> T {
        self.reset()
    }

    fn op(&self) -> Op {
        self.op()
    }
}

impl<T, Op> Variable for Reducer<T, Op>
where
    T: Clone + Send + Sync + fmt::Display + 'static,
    Op: Combiner<T> + Send + Sync + 'static + Clone,
{
    fn describe(&self, f: &mut String, _quote_string: bool) -> bool {
        let _= write!(f, "{}", self.get_value());
        true
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32 {
        // 更新内部名称
        let mut full_name = String::new();
        if !prefix.is_empty() {
            full_name.push_str(prefix);
            full_name.push('_');
        }
        full_name.push_str(name);
        
        // 将自己暴露出去
        let result = <dyn Variable>::default_expose_impl(self, prefix, name);
        if result == 0 {
            // 仅在成功时更新名称
            self.combiner.lock().set_name(full_name);
        }
        result
    }
    
    fn name(&self) -> String {
        self.combiner.lock().name().to_string()
    }
}   

// 常用组合器的实现
use num_traits::NumOps;

use std::marker::PhantomData;
/// 加法操作
#[derive(Clone)]
pub struct AddTo<T>(PhantomData<T>);

impl<T> Default for AddTo<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: NumOps + Clone + Send + Sync + 'static> Combiner<T> for AddTo<T> {
    fn combine(&self, lhs: T, rhs: T) -> T {
        lhs + rhs
    }
    fn modify(&self, v: T) -> T {   
        v
    }
    fn name(&self) -> &'static str {
        "add"
    }
}

/// 减法操作
#[derive(Clone)]
pub struct MinusFrom<T>(PhantomData<T>);

impl<T> Default for MinusFrom<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: NumOps + Clone + Send + Sync + 'static> Combiner<T> for MinusFrom<T> {
    fn combine(&self, lhs: T, rhs: T) -> T {
        lhs - rhs
    }
    fn modify(&self, v: T) -> T {
        v
    }
    fn name(&self) -> &'static str {
        "minus"
    }
}

/// 求和器
pub struct Adder<T> where T: std::ops::Mul<Output = T> + std::ops::Sub<Output = T> + std::ops::Add<Output = T> + std::ops::Rem<Output = T> + std::ops::Div<Output = T> + Clone + Send + Sync + 'static {
    inner: Reducer<T, AddTo<T>>,

}

impl<T> Adder<T>
where
    T: Clone + Send + Sync + fmt::Display + NumOps + Default + 'static,
{
    /// 创建新的加法器
    pub fn new() -> Self {
        Self {
            inner: Reducer::new(T::default(), AddTo::default(), "adder".to_string()),
        }
    }
    
    /// 使用名称创建
    pub fn with_name(name: &str) -> Self {
        let adder = Self::new();
        let _ = adder.expose(name);
        adder
    }
    
    /// 使用前缀和名称创建
    pub fn with_prefix_name(prefix: &str, name: &str) -> Self {
        let adder = Self::new();
        let _ = adder.expose_as(prefix, name);
        adder
    }
    
    /// 添加一个值
    pub fn add(&mut self, value: T) -> &Self {
        self.inner.add(value);
        self
    }
    
    /// 获取当前值
    pub fn get_value(&self) -> T {
        self.inner.get_value()
    }
    
    /// 重置值
    pub fn reset(&self) -> T {
        self.inner.reset()
    }
}

impl<T> Variable for Adder<T>
where
    T: Clone + Send + Sync + fmt::Display + NumOps + Default + 'static,
{
    fn describe(&self, f: &mut String, quote_string: bool) -> bool {
        self.inner.describe(f, quote_string);
        true
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32 {
        self.inner.expose_impl(prefix, name)
    }
    
    fn name(&self) -> String {
        self.inner.name()
    }
}

impl<T> Default for Adder<T>
where
    T: Clone + Send + Sync + fmt::Display + NumOps + Default + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// 求最大值操作
#[derive(Clone)]
pub struct MaxTo<T>(PhantomData<T>);

impl<T> Default for MaxTo<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: PartialOrd + Clone + Send + Sync + 'static> Combiner<T> for MaxTo<T> {
    fn combine(&self, lhs: T, rhs: T) -> T {
        if rhs > lhs {
            rhs
        } else {
            lhs
        }
    }
    fn modify(&self, v: T) -> T {
        v
    }
    fn name(&self) -> &'static str {
        "max"
    }
}

/// 求最大值器
pub struct Maxer<T> where T: PartialOrd + Send + Clone + Sync + 'static {
    inner: Reducer<T, MaxTo<T>>,
}

impl<T> Maxer<T>
where
    T: Clone + Send + Sync + fmt::Display + PartialOrd + 'static,
{
    /// 创建新的最大值器
    pub fn new(default_value: T) -> Self {
        Self {
            inner: Reducer::new(default_value, MaxTo::default(), "maxer".to_string()),
        }
    }
    
    /// 使用名称创建
    pub fn with_name(default_value: T, name: &str) -> Self {
        let maxer = Self::new(default_value);
        let _ = maxer.expose(name);
        maxer
    }
    
    /// 使用前缀和名称创建
    pub fn with_prefix_name(default_value: T, prefix: &str, name: &str) -> Self {
        let maxer = Self::new(default_value);
        let _ = maxer.expose_as(prefix, name);
        maxer
    }
    
    /// 添加一个值
    pub fn add(&mut self, value: T) -> &Self {
        self.inner.add(value);
        self
    }
    
    /// 获取当前值
    pub fn get_value(&self) -> T {
        self.inner.get_value()
    }
    
    /// 重置值
    pub fn reset(&self) -> T {
        self.inner.reset()
    }
}

impl<T> Variable for Maxer<T>
where
    T: Clone + Send + Sync + fmt::Display + PartialOrd + 'static,
{
    fn describe(&self, f: &mut String, quote_string: bool) -> bool {
        self.inner.describe(f, quote_string);
        true
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32 {
        self.inner.expose_impl(prefix, name)
    }
    
    fn name(&self) -> String {
        self.inner.name()
    }
}

/// 求最小值操作
#[derive(Clone)]
pub struct MinTo<T>(PhantomData<T>);

impl<T> Default for MinTo<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: PartialOrd + Clone + Send + Sync + 'static> Combiner<T> for MinTo<T> {
    fn combine(&self, lhs: T, rhs: T) -> T {
        if rhs < lhs {
            rhs
        } else {
            lhs
        }
    }
    fn modify(&self, v: T) -> T {
        v
    }
    fn name(&self) -> &'static str {
        "min"
    }
}

/// 求最小值器
pub struct Miner<T> where T: PartialOrd + Clone + Send + Sync + 'static {
    inner: Reducer<T, MinTo<T>>,
}

impl<T> Miner<T>
where
    T: Clone + Send + Sync + fmt::Display + PartialOrd + 'static,
{
    /// 创建新的最小值器
    pub fn new(default_value: T) -> Self {
        Self {
            inner: Reducer::new(default_value, MinTo::default(), "miner".to_string()),
        }
    }
    
    /// 使用名称创建
    pub fn with_name(default_value: T, name: &str) -> Self {
        let miner = Self::new(default_value);
        let _ = miner.expose(name);
        miner
    }
    
    /// 使用前缀和名称创建
    pub fn with_prefix_name(default_value: T, prefix: &str, name: &str) -> Self {
        let miner = Self::new(default_value);
        let _ = miner.expose_as(prefix, name);
        miner
    }
    
    /// 添加一个值
    pub fn add(&mut self, value: T) -> &Self {
        self.inner.add(value);
        self
    }
    
    /// 获取当前值
    pub fn get_value(&self) -> T {
        self.inner.get_value()
    }
    
    /// 重置值
    pub fn reset(&self) -> T {
        self.inner.reset()
    }
}

impl<T> Variable for Miner<T>
where
    T: Clone + Send + Sync + fmt::Display + PartialOrd + 'static,
{
    fn describe(&self, f: &mut String, quote_string: bool) ->  bool {
        self.inner.describe(f, quote_string);
        true
    }
    
    fn expose_impl(&self, prefix: &str, name: &str) -> i32 {
        self.inner.expose_impl(prefix, name)
    }
    
    fn name(&self) -> String {
        self.inner.name()
    }
}

/// 提供求和操作
#[derive(Clone)]
pub struct SumCombiner;

impl<T> Combiner<T> for SumCombiner
where
    T: std::ops::Add<Output = T> + Clone,
{
    fn combine(&self, v1: T, v2: T) -> T {
        v1 + v2
    }
    
    fn modify(&self, v: T) -> T {
        v
    }
    
    fn name(&self) -> &'static str {
        "sum"
    }
}

/// 提供求最大值操作
#[derive(Clone)]
pub struct MaxCombiner;

impl<T> Combiner<T> for MaxCombiner
where
    T: std::cmp::PartialOrd + Clone,
{
    fn combine(&self, v1: T, v2: T) -> T {
        if v1 > v2 {
            v1
        } else {
            v2
        }
    }
    
    fn modify(&self, v: T) -> T {
        v
    }
    
    fn name(&self) -> &'static str {
        "max"
    }
}

/// 提供求最小值操作
#[derive(Clone)]
pub struct MinCombiner;

impl<T> Combiner<T> for MinCombiner
where
    T: std::cmp::PartialOrd + Clone,
{
    fn combine(&self, v1: T, v2: T) -> T {
        if v1 < v2 {
            v1
        } else {
            v2
        }
    }
    
    fn modify(&self, v: T) -> T {
        v
    }
    
    fn name(&self) -> &'static str {
        "min"
    }
}

// /// 提供求平均值操作
// #[derive(Clone)]
// pub struct AvgCombiner {
//     /// 当前总和
//     sum: AtomicI64,
//     /// 当前计数
//     count: AtomicUsize,
// }

// impl AvgCombiner {
//     /// 创建新的平均值组合器
//     pub fn new() -> Self {
//         Self {
//             sum: AtomicI64::new(0),
//             count: AtomicUsize::new(0),
//         }
//     }
// }
// use std::sync::atomic::Ordering;
// use std::sync::atomic::AtomicUsize;
// use std::sync::atomic::AtomicI64;
// impl Combiner<i64> for AvgCombiner {
//     fn combine(&self, _v1: i64, v2: i64) -> i64 {
//         self.sum.fetch_add(v2, Ordering::Relaxed);
//         self.count.fetch_add(1, Ordering::Relaxed);
        
//         let sum = self.sum.load(Ordering::Relaxed);
//         let count = self.count.load(Ordering::Relaxed);
        
//         if count > 0 {
//             sum / count as i64
//         } else {
//             0
//         }
//     }
    
//     fn modify(&self, v: i64) -> i64 {
//         v
//     }
    
//     fn name(&self) -> &'static str {
//         "avg"
//     }
// } 

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable::Variable;


    #[test]
    fn test_reducer() {
        let mut reducer = Reducer::new(0, AddTo::default(), "test".to_string());
        let _ = reducer.expose("test");
        let _ = reducer.expose_as("prefix", "test");
        let _ = reducer.add(1);
        let _ = reducer.add(2);
        let _ = reducer.add(3);
        let _ = reducer.add(4);
        let _ = reducer.add(5);
        let _ = reducer.add(6);
        let _ = reducer.add(7);
        let _ = reducer.reset();
    }   
}