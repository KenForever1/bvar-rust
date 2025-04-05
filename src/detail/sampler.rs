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

//! 实现对变量进行定期采样的功能

use std::fmt;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use once_cell::sync::Lazy;

use crate::detail::combiner::Combiner;
use crate::window::SERIES_IN_SECOND;
use super::combiner::SampleErrorHandler;
use crate::reducer::ReducerTrait;
/// 全局采样器的状态
pub static GLOBAL_SAMPLER_STATE: Lazy<Arc<Mutex<GlobalSamplerState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(GlobalSamplerState {
        samplers: Vec::new(),
        last_sample_time: Instant::now(),
        sample_interval: Duration::from_secs(1),
        is_running: false,
    }))
});

/// 采样器特性
pub trait Sampler: Send + Sync + 'static {
    /// 获取采样间隔
    fn interval(&self) -> Duration;
    
    /// 执行一次采样
    fn take_sample(&self);
    
    /// 描述采样内容
    fn describe(&self, f: &mut dyn fmt::Write);
    
    /// 销毁采样器
    fn destroy(&self);
}

/// 全局采样器状态
pub struct GlobalSamplerState {
    /// 所有注册的采样器
    samplers: Vec<Weak<dyn Sampler>>,
    /// 上次采样时间
    last_sample_time: Instant,
    /// 采样间隔
    sample_interval: Duration,
    /// 采样线程是否运行中
    is_running: bool,
}

impl GlobalSamplerState {
    /// 注册一个新的采样器
    pub fn register_sampler(&mut self, sampler: Weak<dyn Sampler>) {
        // 清理已失效的采样器
        self.samplers.retain(|s| s.upgrade().is_some());
        
        // 添加新采样器
        self.samplers.push(sampler);

        self.samplers.iter().for_each(|s| {
            println!("call GlobalSamplerState::register_sampler s.upgrade().is_some(): {}", s.upgrade().is_some());
        });
        
        // 如果还没有启动线程，则启动
        if !self.is_running {
            self.start_sampler_thread();
        }
    }
    
    /// 启动采样线程
    fn start_sampler_thread(&mut self) {
        println!("call GlobalSamplerState::start_sampler_thread");
        if self.is_running {
            return;
        }
        println!("call GlobalSamplerState::start_sampler_thread is_running: {}", self.is_running);
        
        self.is_running = true;
        
        // 克隆一份状态用于线程
        let state = GLOBAL_SAMPLER_STATE.clone();
        
        // 启动后台线程
        thread::spawn(move || {
            println!("call GlobalSamplerState::start_sampler_thread thread::spawn");
            loop {
                // 睡眠一段时间
                thread::sleep(Duration::from_millis(100));
                
                // 检查是否需要采样
                let mut guard = state.lock();

                guard.samplers.iter().for_each(|s| {
                    println!("call GlobalSamplerState::start_sampler_thread s.upgrade().is_some(): {}", s.upgrade().is_some());
                });

                let now = Instant::now();
                
                println!("call GlobalSamplerState::start_sampler_thread now");
                if now.duration_since(guard.last_sample_time) >= guard.sample_interval {
                    println!("call GlobalSamplerState::start_sampler_thread now.duration_since(guard.last_sample_time) >= guard.sample_interval");
                    guard.last_sample_time = now;
                    

                    println!("len of samplers: {}", guard.samplers.len());
                    // 获取所有有效的采样器
                    let valid_samplers: Vec<_> = guard.samplers
                        .iter()
                        .filter_map(|s| {
                            // if s.upgrade().is_none() {
                            //     println!("call GlobalSamplerState::start_sampler_thread s.upgrade().is_none()");
                            //     return None;
                            // }
                            s.upgrade()
                        }    
                        )
                        .collect();
                    
                    println!("call GlobalSamplerState::start_sampler_thread valid_samplers: {}", valid_samplers.len());
                    // 释放锁，避免在采样期间持有锁
                    drop(guard);
                    
                    // 对每个采样器执行采样
                    for sampler in valid_samplers {
                        println!("call GlobalSamplerState::start_sampler_thread for sampler");
                        sampler.take_sample();
                    }
                    
                    // 清理无效的采样器
                    let mut guard = state.lock();
                    guard.samplers.retain(|s| s.upgrade().is_some());
                    
                    // 如果没有采样器了，退出线程
                    if guard.samplers.is_empty() {
                        guard.is_running = false;
                        break;
                    }
                }
            }
        });
    }
}


/// 采样器
pub struct ReducerSampler<Owner, T, Op, InvOp> where
T: Clone + Send + Sync,
Op: Clone + Combiner<T> + Send + Sync + 'static,
InvOp: Clone + Send + Sync + 'static,
Owner: Clone + Send + Sync + 'static + ReducerTrait<T, Op>,
{
    /// 变量所有者
    owner: Weak<Owner>,
    /// 是否已经销毁
    destroyed: AtomicBool,
    /// 使用的操作
    op: Op,
    /// 反向操作
    inv_op: InvOp,
    /// 错误处理
    error_handler: Arc<dyn SampleErrorHandler>,
    _marker: std::marker::PhantomData<T>,
    weak_self: Mutex<Option<Weak<dyn Sampler + 'static + Send + Sync>>>,
}

use crate::detail::combiner::LoggingErrorHandler;

impl<Owner, T, Op, InvOp> ReducerSampler<Owner, T, Op, InvOp>
where
    Owner: Clone + Send + Sync + 'static + ReducerTrait<T, Op>,
    T: Clone + Send + Sync + 'static,
    Op: Clone + Combiner<T> + Send + Sync + 'static,
    InvOp: Clone + Send + Sync + 'static,
{
    /// 创建新的采样器
    pub fn new(owner: &Arc<Owner>, op: Op, inv_op: InvOp) -> Arc<Self> {

        // this is error
        // let self_arc = Arc::new(Self {
        //     owner: Arc::downgrade(owner),
        //     destroyed: AtomicBool::new(false),
        //     op,
        //     inv_op,
        //     error_handler: Arc::new(super::combiner::LoggingErrorHandler),
        //     _marker: std::marker::PhantomData,
        //     weak_self: Arc::new(Mutex::new(None)),
        // });
        // // 生成 Weak 指针并存入结构体
        // let weak = Arc::downgrade(&self_arc);
        // self_arc.weak_self.lock().unwrap() = weak;
        // self_arc

        // 通过 new_cyclic 捕获 weak 指针
        Arc::new_cyclic(|weak| -> Self {
            Self {
                owner: Arc::downgrade(owner),
                destroyed: AtomicBool::new(false),
                op,
                inv_op,
                error_handler: Arc::new(LoggingErrorHandler),
                _marker: std::marker::PhantomData,
                // 立即存入初始化时的 weak 指针
                weak_self: Mutex::new(Some(weak.clone())),
            }
        })
    }
    
    /// 安排采样任务
    pub fn schedule(&self) -> bool {

        // 安全获取弱引用
        if let Some(weak) = &*self.weak_self.lock() {
            GLOBAL_SAMPLER_STATE
                .lock()
                .register_sampler(weak.clone());
            return true;
        }
        
        false
    }

}

impl<Owner, T, Op, InvOp> Clone for ReducerSampler<Owner, T, Op, InvOp>
where
    Owner: Clone + Send + Sync + 'static + ReducerTrait<T, Op>,
    T: Clone + Send + Sync + 'static,
    Op: Combiner<T> + Clone + Send + Sync + 'static,
    InvOp: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            destroyed: AtomicBool::new(self.destroyed.load(Ordering::Relaxed)),
            op: self.op.clone(),
            inv_op: self.inv_op.clone(),
            error_handler: self.error_handler.clone(),
            _marker: std::marker::PhantomData,
            // 特殊处理 weak_self：克隆内部的 Weak 指针
            weak_self: Mutex::new(
                self.weak_self
                    .lock()
                    // .unwrap()
                    .as_ref()
                    .map(|w| w.clone())
            ),
        }
    }
}

impl<Owner, T, Op, InvOp> Sampler for ReducerSampler<Owner, T, Op, InvOp>
where
    Owner: Clone + Send + Sync + 'static + ReducerTrait<T, Op>,
    T: Clone + Send + Sync + 'static,
    Op: Clone + Combiner<T> + Send + Sync + 'static,
    InvOp: Clone + Send + Sync + 'static,
{
    fn interval(&self) -> Duration {
        Duration::from_secs(1)
    }
    
    fn take_sample(&self) {
        if self.destroyed.load(Ordering::Relaxed) {
            return;
        }
        
        // 从owner中获取值并记录
        if let Some(owner) = self.owner.upgrade() {
            // 获取当前值
            let value = owner.get_value();

            println!("call ReducerSampler::take_sample");
            
            // 使用op进行组合
            let _ = self.op.combine(value.clone(), value);
            
            // 记录错误
            self.error_handler.on_error("采样错误");
        }
    }
    
    fn describe(&self, _f: &mut dyn fmt::Write) {
        // 实现样本描述
    }
    
    fn destroy(&self) {
        self.destroyed.store(true, Ordering::Relaxed);
    }
}

/// 一系列采样数据，用于记录和可视化
pub struct SeriesSampler<T, Op> {
    /// 序列数据
    data: Mutex<Vec<T>>,
    /// 组合操作
    op: Op,
    /// 是否已销毁
    destroyed: AtomicBool,
}

impl<T, Op> SeriesSampler<T, Op>
where
    T: Clone + Send + Sync + 'static,
    Op: Combiner<T> + Send + Sync + Clone + 'static,
{
    /// 创建新的序列采样器
    pub fn new(op: Op) -> Self {
        Self {
            data: Mutex::new(Vec::with_capacity(SERIES_IN_SECOND)),
            op,
            destroyed: AtomicBool::new(false),
        }
    }
    
    /// 添加一个样本
    pub fn append(&self, value: T) {
        if self.destroyed.load(Ordering::Relaxed) {
            return;
        }
        
        let mut data = self.data.lock();
        
        // 添加新值
        data.push(value);
        
        // 如果超过容量，移除最旧的值
        if data.len() > SERIES_IN_SECOND {
            data.remove(0);
        }
    }
    
    /// 安排采样任务
    pub fn schedule(&self) -> bool {
        true
    }
}

impl<T, Op> Sampler for SeriesSampler<T, Op>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    Op: Combiner<T> + Send + Sync + Clone + 'static,
{
    fn interval(&self) -> Duration {
        Duration::from_secs(1)
    }
    
    fn take_sample(&self) {
        // 序列采样器不需要主动采样
    }
    
    fn describe(&self, f: &mut dyn fmt::Write) {
        if self.destroyed.load(Ordering::Relaxed) {
            return;
        }
        
        let data = self.data.lock();
        
        // 将数据格式化为JSON数组
        let _ = write!(f, "[");
        
        let mut first = true;
        for value in data.iter() {
            if !first {
                let _ = write!(f, ",");
            }
            first = false;
            
            // 这里需要根据T类型进行适当格式化
            let _ = write!(f, "{:?}", value);
        }
        
        let _ = write!(f, "]");
    }
    
    fn destroy(&self) {
        self.destroyed.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::AddTo;
    use crate::reducer::Reducer;
    use crate::reducer::VoidOp;

    #[test]
    fn test_sampler() {
        let sampler : Arc<ReducerSampler<Reducer<i32, AddTo<i32>>, i32, AddTo<_>, VoidOp>> = ReducerSampler::new(
            &Arc::new(Reducer::new(0, AddTo::default(), "adder".to_string())),
            AddTo::default(), 
            VoidOp
        );
        sampler.schedule();

        thread::sleep(Duration::from_secs(10));

        println!("Sampler finished")

        // let series: <dyn Sampler>::SeriesSampler<_, Reducer::AddTo<_>> = SeriesSampler::new(AddTo::default());
        // series.schedule();
    }
}
