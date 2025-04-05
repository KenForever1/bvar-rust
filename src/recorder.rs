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

//! 用于计算数值的平均值

use std::fmt;
use thread_local::ThreadLocal;
use parking_lot::Mutex;
use crate::variable::Variable;
use std::fmt::Write;
use std::cell::UnsafeCell;
/// 统计结构，用于计算平均值
#[derive(Debug, Clone, Default)]
pub struct Stat {
    /// 值的总和
    pub sum: i64,
    /// 值的数量
    pub num: i64,
}

impl Stat {
    /// 创建新的统计结构
    pub fn new(sum: i64, num: i64) -> Self {
        Self { sum, num }
    }
    
    /// 获取整数平均值
    pub fn get_average_int(&self) -> i64 {
        if self.num == 0 {
            return 0;
        }
        self.sum / self.num
    }
    
    /// 获取浮点数平均值
    pub fn get_average_double(&self) -> f64 {
        if self.num == 0 {
            return 0.0;
        }
        self.sum as f64 / self.num as f64
    }
}

impl std::ops::Sub for Stat {
    type Output = Self;
    
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            sum: self.sum - rhs.sum,
            num: self.num - rhs.num,
        }
    }
}

impl std::ops::SubAssign for Stat {
    fn sub_assign(&mut self, rhs: Self) {
        self.sum -= rhs.sum;
        self.num -= rhs.num;
    }
}

impl std::ops::Add for Stat {
    type Output = Self;
    
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            sum: self.sum + rhs.sum,
            num: self.num + rhs.num,
        }
    }
}

impl std::ops::AddAssign for Stat {
    fn add_assign(&mut self, rhs: Self) {
        self.sum += rhs.sum;
        self.num += rhs.num;
    }
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let v = self.get_average_int();
        if v != 0 {
            write!(f, "{}", v)
        } else {
            write!(f, "{}", self.get_average_double())
        }
    }
}

#[derive(Debug)]
/// 线程本地的Agent
struct Agent {
    value: Stat,
}

/// 用于计算整数平均值的记录器
#[derive(Debug)]
pub struct IntRecorder {
    /// 线程本地存储
    tls: ThreadLocal<Mutex<Agent>>,
    /// 变量名称
    name: UnsafeCell<String>,
    /// 用于调试的名称
    debug_name: String,
}

// 手动实现线程安全 - 我们确保对UnsafeCell的访问是安全的
unsafe impl Send for IntRecorder {}
unsafe impl Sync for IntRecorder {}

impl IntRecorder {
    /// 创建一个新的整数记录器
    pub fn new() -> Self {
        Self {
            tls: ThreadLocal::new(),
            name: UnsafeCell::new(String::new()),
            debug_name: String::new(),
        }
    }
    
    /// 用名称创建
    pub fn with_name(name: &str) -> Self {
        let recorder = Self::new();
        let _ = recorder.expose(name);
        recorder
    }
    
    /// 用前缀和名称创建
    pub fn with_prefix_name(prefix: &str, name: &str) -> Self {
        let recorder = Self::new();
        let _ = recorder.expose_as(prefix, name);
        recorder
    }
    
    /// 添加一个样本
    pub fn add(&self, sample: i32) -> &Self {
        // 获取或创建线程本地值
        let agent = self.tls.get_or(|| {
            Mutex::new(Agent {
                value: Stat::default(),
            })
        });
        
        // 更新值
        let mut guard = agent.lock();
        guard.value.sum += sample as i64;
        guard.value.num += 1;
        
        self
    }
    
    /// 获取整数平均值
    pub fn average(&self) -> i64 {
        self.get_value().get_average_int()
    }
    
    /// 获取浮点数平均值
    pub fn average_double(&self) -> f64 {
        self.get_value().get_average_double()
    }
    
    /// 获取当前统计值
    pub fn get_value(&self) -> Stat {
        let mut result = Stat::default();
        
        for agent in self.tls.iter() {
            let agent_value = agent.lock().value.clone();
            result += agent_value;
        }
        
        result
    }
    
    /// 重置所有值
    pub fn reset(&self) -> Stat {
        let result = self.get_value();
        
        for agent in self.tls.iter() {
            let mut guard = agent.lock();
            guard.value = Stat::default();
        }
        
        result
    }
    
    /// 设置用于调试的名称
    pub fn set_debug_name(&mut self, name: &str) {
        self.debug_name = name.to_string();
    }
}

impl Variable for IntRecorder {
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
        println!("expose_impl: {}", name);
        full_name.push_str(name);
        
        // 将自己暴露出去
        // let result = <dyn Variable>::default_expose_impl(self, prefix, name);
        // let result = Variable::default_expose_impl(self, prefix, name);
        let result = <IntRecorder as Variable>::default_expose_impl(&self, prefix, name);
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

impl Default for IntRecorder {
    fn default() -> Self {
        Self::new()
    }
} 

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_int_recorder() {
        let recorder = IntRecorder::new();
        let _ = recorder.expose("test");
        let value = recorder.get_value();
        assert_eq!(value.sum , 0);
        assert_eq!(value.num , 0);

        recorder.add(1);
        let value = recorder.get_value();
        assert_eq!(value.sum , 1);
        assert_eq!(value.num , 1);

        recorder.add(2);
        let value = recorder.get_value();
        assert_eq!(value.sum , 3);
        assert_eq!(value.num , 2);

        recorder.reset();
        let value = recorder.get_value();
        assert_eq!(value.sum , 0);
        assert_eq!(value.num , 0);
        
    }
}