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

//! 实现时间窗口统计功能

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use std::fmt::Write;
use std::cell::UnsafeCell;

use crate::variable::Variable;

/// 表示一个时间窗口内的数据样本
struct Sample<T> {
    /// 样本数据
    value: T,
    /// 采样时间
    time: Instant,
}

/// 默认的秒级窗口大小 (60秒)
pub const WINDOW_SIZE_SECOND: u64 = 60;
/// 默认的分钟级窗口大小 (60分钟)
pub const WINDOW_SIZE_MINUTE: u64 = 60;
/// 默认的小时级窗口大小 (24小时)
pub const WINDOW_SIZE_HOUR: u64 = 24;
/// 默认的天级窗口大小 (30天)
pub const WINDOW_SIZE_DAY: u64 = 30;

/// 秒级序列的最大数据点数量
pub const SERIES_IN_SECOND: usize = WINDOW_SIZE_SECOND as usize;
/// 分钟级序列的最大数据点数量
pub const SERIES_IN_MINUTE: usize = WINDOW_SIZE_MINUTE as usize;
/// 小时级序列的最大数据点数量
pub const SERIES_IN_HOUR: usize = WINDOW_SIZE_HOUR as usize;
/// 天级序列的最大数据点数量
pub const SERIES_IN_DAY: usize = WINDOW_SIZE_DAY as usize;

/// 表示一个时间窗口，用于记录和统计时间窗口内的数据
pub struct Window<T, const N: usize> {
    /// 数据源
    source: Arc<dyn Variable>,
    /// 采样间隔
    interval: Duration,
    /// 样本数据
    samples: RwLock<Vec<Sample<T>>>,
    /// 变量名称
    name: UnsafeCell<String>,
    /// 最近一次采样时间
    last_sample_time: RwLock<Instant>,
    /// 标记类型
    _marker: PhantomData<T>,
}

// 手动实现线程安全 - 我们确保对UnsafeCell的访问是安全的
unsafe impl<T, const N: usize> Send for Window<T, N> {}
unsafe impl<T, const N: usize> Sync for Window<T, N> {}

impl<T, const N: usize> Window<T, N>
where
    T: Clone + fmt::Display + Send + Sync + 'static,
{
    /// 创建新的时间窗口
    pub fn new<S>(source: &S, interval_seconds: u64) -> Self
    where
        S: Variable + Clone + 'static,
    {
        Self {
            source: Arc::new(source.clone()),
            interval: Duration::from_secs(interval_seconds),
            samples: RwLock::new(Vec::with_capacity(N)),
            name: UnsafeCell::new(String::new()),
            last_sample_time: RwLock::new(Instant::now()),
            _marker: PhantomData,
        }
    }
    
    /// 用名称创建
    pub fn with_name<S>(name: &str, source: &S, interval_seconds: u64) -> Self
    where
        S: Variable + Clone + 'static,
    {
        let window = Self::new(source, interval_seconds);
        let _ = window.expose(name);
        window
    }
    
    /// 获取当前值
    pub fn get_value(&self) -> Option<T> {
        // 实现窗口内的数据统计
        // 这里简单返回最新的样本
        let samples = self.samples.read();
        if let Some(sample) = samples.last() {
            Some(sample.value.clone())
        } else {
            None
        }
    }
    
    /// 添加新的样本
    fn add_sample(&self, value: T) {
        let now = Instant::now();
        let mut samples = self.samples.write();
        
        // 添加新样本
        samples.push(Sample { value, time: now });
        
        // 移除过期样本
        let cutoff = now - self.interval * N as u32;
        while samples.len() > N || (samples.len() > 0 && samples[0].time < cutoff) {
            samples.remove(0);
        }
        
        // 更新最后采样时间
        *self.last_sample_time.write() = now;
    }
    
    /// 触发采样
    pub fn sample(&self) {
        // 实际产品中此处需要根据T类型从source获取值
        // let value = self.source.get_value();
        // self.add_sample(value);
        // println!("sample: {}", value);
    }
}

impl<T, const N: usize> Variable for Window<T, N>
where
    T: Clone + fmt::Display + Send + Sync + 'static,
{
    fn describe(&self, f: &mut String, quote_string: bool) -> bool {
        if let Some(value) = self.get_value() {
            if quote_string && std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
                let _ = write!(f, "\"{}\"", value);
            } else {
                let _ = write!(f, "{}", value);
            }
        } else {
            let _ = write!(f, "N/A");
        }
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
        let result = <Window<T, N> as Variable>::default_expose_impl(self, prefix, name);
        if result == 0 {
            // 仅在成功时更新名称
            // 使用UnsafeCell安全地更新内部状态
            unsafe {
                *self.name.get() = full_name;
            }
        }
        result
    }
    
    fn name(&self) -> String {
        unsafe { (*self.name.get()).clone() }
    }
}

/// 表示单位时间内的操作次数
pub struct PerSecond<T> {
    /// 内部窗口
    window: Window<f64, SERIES_IN_SECOND>,
    /// 上次统计的值
    last_value: RwLock<Option<T>>,
    /// 上次统计的时间
    last_time: RwLock<Instant>,
    /// 变量名称
    name: UnsafeCell<String>,
}

// 手动实现线程安全 - 我们确保对UnsafeCell的访问是安全的
unsafe impl<T> Send for PerSecond<T> {}
unsafe impl<T> Sync for PerSecond<T> {}

impl<T> PerSecond<T>
where
    T: Clone + fmt::Display + Send + Sync + 'static,
{
    /// 创建新的QPS统计器
    pub fn new<S>(source: &S) -> Self
    where
        S: Variable + Clone + 'static,
    {
        Self {
            window: Window::new(source, 1),
            last_value: RwLock::new(None),
            last_time: RwLock::new(Instant::now()),
            name: UnsafeCell::new(String::new()),
        }
    }
    
    /// 用名称创建
    pub fn with_name<S>(name: &str, source: &S) -> Self
    where
        S: Variable + Clone + 'static,
    {
        let per_second = Self::new(source);
        let _ = per_second.expose(name);
        per_second
    }
    
    /// 获取当前QPS
    pub fn get_value(&self) -> f64 {
        // 实际产品中需要根据T类型计算QPS
        // 这里简单返回窗口中的平均值
        self.window.get_value().unwrap_or(0.0)
    }
}

impl<T> Variable for PerSecond<T>
where
    T: Clone + fmt::Display + Send + Sync + 'static,
{
    fn describe(&self, f: &mut String, _quote_string: bool) -> bool {
        let _ = write!(f, "{}", self.get_value());
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
        let result = <PerSecond<T> as Variable>::default_expose_impl(self, prefix, name);
        if result == 0 {
            // 仅在成功时更新名称
            // 使用UnsafeCell安全地更新内部状态
            unsafe {
                *self.name.get() = full_name;
            }
            
            // 同时暴露内部窗口
            let window_name = format!("{}_second", name);
            let _ = self.window.expose_as(prefix, &window_name);
        }
        result
    }
    
    fn name(&self) -> String {
        unsafe { (*self.name.get()).clone() }
    }
}

/// 返回当前的Unix时间戳（毫秒）
pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

/// 时间窗口定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// 10秒内窗口
    Second10,
    /// 1分钟窗口
    Minute1,
    /// 5分钟窗口
    Minute5,
    /// 15分钟窗口
    Minute15,
    /// 1小时窗口
    Hour1,
    /// 6小时窗口
    Hour6,
    /// 12小时窗口
    Hour12,
    /// 1天窗口
    Day1,
    /// 7天窗口
    Day7,
    /// 30天窗口
    Day30,
}

impl WindowType {
    /// 返回窗口持续时间（秒）
    pub fn duration_secs(&self) -> u64 {
        match self {
            WindowType::Second10 => 10,
            WindowType::Minute1 => 60,
            WindowType::Minute5 => 300,
            WindowType::Minute15 => 900,
            WindowType::Hour1 => 3600,
            WindowType::Hour6 => 21600,
            WindowType::Hour12 => 43200,
            WindowType::Day1 => 86400,
            WindowType::Day7 => 604800,
            WindowType::Day30 => 2592000,
        }
    }
    
    /// 返回窗口持续时间
    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.duration_secs())
    }
    
    /// 检查给定的时间点是否在当前窗口内
    pub fn contains(&self, time: Instant, now: Instant) -> bool {
        if now < time {
            return false;
        }
        
        let elapsed = now.duration_since(time);
        elapsed <= self.duration()
    }
    
    /// 获取窗口的显示名称
    pub fn name(&self) -> &'static str {
        match self {
            WindowType::Second10 => "10_second",
            WindowType::Minute1 => "1_minute",
            WindowType::Minute5 => "5_minute",
            WindowType::Minute15 => "15_minute",
            WindowType::Hour1 => "1_hour",
            WindowType::Hour6 => "6_hour",
            WindowType::Hour12 => "12_hour",
            WindowType::Day1 => "1_day",
            WindowType::Day7 => "7_day",
            WindowType::Day30 => "30_day",
        }
    }
}

/// 定义常用窗口的迭代器
pub struct CommonWindows;

impl CommonWindows {
    /// 返回所有常用窗口的迭代器
    pub fn iter() -> impl Iterator<Item = WindowType> {
        [
            WindowType::Minute1,
            WindowType::Minute5,
            WindowType::Minute15,
            WindowType::Hour1,
            WindowType::Hour6,
            WindowType::Hour12,
            WindowType::Day1,
        ].into_iter()
    }
    
    /// 返回所有窗口的迭代器
    pub fn all() -> impl Iterator<Item = WindowType> {
        [
            WindowType::Second10,   
            WindowType::Minute1,
            WindowType::Minute5,
            WindowType::Minute15,
            WindowType::Hour1,
            WindowType::Hour6,
            WindowType::Hour12,
            WindowType::Day1,
            WindowType::Day7,
            WindowType::Day30,
        ].into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    
    #[test]
    fn test_window_duration() {
        assert_eq!(WindowType::Second10.duration_secs(), 10);
        assert_eq!(WindowType::Minute1.duration_secs(), 60);
        assert_eq!(WindowType::Minute5.duration_secs(), 300);
        assert_eq!(WindowType::Hour1.duration_secs(), 3600);
        assert_eq!(WindowType::Day1.duration_secs(), 86400);
    }
    
    #[test]
    fn test_window_contains() {
        let now = Instant::now();
        sleep(Duration::from_millis(10));
        let future = Instant::now();
        
        // 当前时间在窗口内
        assert!(WindowType::Second10.contains(now, now));
        assert!(WindowType::Minute1.contains(now, now));
        
        // 未来时间不在窗口内
        assert!(!WindowType::Second10.contains(future, now));
        
        // 等待一小段时间，但仍在10秒窗口内
        sleep(Duration::from_millis(50));
        let later = Instant::now();
        assert!(WindowType::Second10.contains(now, later));
        
        assert!(WindowType::Second10.contains(future, later));
    }
    
    #[test]
    fn test_window_name() {
        assert_eq!(WindowType::Second10.name(), "10_second");
        assert_eq!(WindowType::Minute1.name(), "1_minute");
        assert_eq!(WindowType::Hour1.name(), "1_hour");
        assert_eq!(WindowType::Day1.name(), "1_day");
    }
    
    #[test]
    fn test_common_windows() {
        let windows: Vec<WindowType> = CommonWindows::iter().collect();
        assert_eq!(windows.len(), 7);
        assert!(windows.contains(&WindowType::Minute1));
        assert!(windows.contains(&WindowType::Hour1));
        assert!(windows.contains(&WindowType::Day1));
        
        let all_windows: Vec<WindowType> = CommonWindows::all().collect();
        assert_eq!(all_windows.len(), 10);
    }
    
    #[test]
    fn test_current_time_ms() {
        let t1 = current_time_ms();
        sleep(Duration::from_millis(10));
        let t2 = current_time_ms();
        assert!(t2 >= t1);
    }
} 