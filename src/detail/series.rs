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

//! 实现时间序列数据存储和展示

use std::fmt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;

use crate::variable::SeriesOptions;
use crate::window::{SERIES_IN_SECOND, SERIES_IN_MINUTE, SERIES_IN_HOUR, SERIES_IN_DAY};

use crate::detail::combiner::Combiner;

/// 表示采样的数据点
#[derive(Clone, Debug)]
pub struct DataPoint<T> {
    /// 数据值
    pub value: T,
    /// 时间戳（Unix时间戳，毫秒）
    pub timestamp: u64,
}

impl<T> DataPoint<T> {
    /// 创建新的数据点
    pub fn new(value: T, timestamp: u64) -> Self {
        Self { value, timestamp }
    }
    
    /// 使用当前时间创建数据点
    pub fn now(value: T) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64;
        
        Self { value, timestamp: now }
    }
}

/// 表示一个时间序列
pub struct Series<T, Op> {
    /// 秒级数据，最近60秒
    second_points: RwLock<Vec<DataPoint<T>>>,
    /// 分钟级数据，最近60分钟
    minute_points: RwLock<Vec<DataPoint<T>>>,
    /// 小时级数据，最近24小时
    hour_points: RwLock<Vec<DataPoint<T>>>,
    /// 天级数据，最近30天
    day_points: RwLock<Vec<DataPoint<T>>>,
    /// 组合操作符
    op: Op,
    /// 最后一次添加的数据点
    last_point: RwLock<Option<DataPoint<T>>>,
    /// 上次采样时间
    last_sample_time: RwLock<Option<Instant>>,
}

impl<T, Op> Series<T, Op>
where
    T: Clone + fmt::Debug + Send + Sync + 'static,
    Op: Combiner<T> + Clone + Send + Sync + 'static,
{
    /// 创建新的时间序列
    pub fn new(op: Op) -> Self {
        Self {
            second_points: RwLock::new(Vec::with_capacity(SERIES_IN_SECOND)),
            minute_points: RwLock::new(Vec::with_capacity(SERIES_IN_MINUTE)),
            hour_points: RwLock::new(Vec::with_capacity(SERIES_IN_HOUR)),
            day_points: RwLock::new(Vec::with_capacity(SERIES_IN_DAY)),
            op,
            last_point: RwLock::new(None),
            last_sample_time: RwLock::new(None),
        }
    }
    
    /// 添加一个数据点
    pub fn append(&self, value: T) {
        let point = DataPoint::now(value);
        
        // 更新最后的数据点
        *self.last_point.write() = Some(point.clone());
        
        // 检查是否需要采样
        let now = Instant::now();
        let mut should_sample_second = true;
        let mut should_sample_minute = false;
        let mut should_sample_hour = false;
        let mut should_sample_day = false;
        
        if let Some(last_time) = *self.last_sample_time.read() {
            let elapsed = now.duration_since(last_time);
            should_sample_second = elapsed >= Duration::from_secs(1);
            should_sample_minute = elapsed >= Duration::from_secs(60);
            should_sample_hour = elapsed >= Duration::from_secs(3600);
            should_sample_day = elapsed >= Duration::from_secs(86400);
        }
        
        // 更新采样时间
        if should_sample_second {
            *self.last_sample_time.write() = Some(now);
        }
        
        // 添加到不同时间粒度的序列中
        if should_sample_second {
            self.add_to_series(&self.second_points, point.clone(), SERIES_IN_SECOND);
        }
        
        if should_sample_minute {
            self.add_to_series(&self.minute_points, point.clone(), SERIES_IN_MINUTE);
        }
        
        if should_sample_hour {
            self.add_to_series(&self.hour_points, point.clone(), SERIES_IN_HOUR);
        }
        
        if should_sample_day {
            self.add_to_series(&self.day_points, point, SERIES_IN_DAY);
        }
    }
    
    /// 添加到指定的时间序列
    fn add_to_series(&self, series: &RwLock<Vec<DataPoint<T>>>, point: DataPoint<T>, max_size: usize) {
        let mut series = series.write();
        
        // 添加新点
        series.push(point);
        
        // 如果超过容量，移除最旧的点
        if series.len() > max_size {
            series.remove(0);
        }
    }
    
    /// 获取最后一个数据点
    pub fn last_point(&self) -> Option<DataPoint<T>> {
        self.last_point.read().clone()
    }
    
    /// 描述序列数据为JSON格式
    pub fn describe(&self, f: &mut dyn fmt::Write, options: &SeriesOptions) {
        // 获取所有数据序列的快照
        let second_points = self.second_points.read().clone();
        let minute_points = self.minute_points.read().clone();
        let hour_points = self.hour_points.read().clone();
        let day_points = self.day_points.read().clone();
        
        // 创建JSON对象
        let _ = write!(f, "{{");
        
        // 添加元数据
        let _ = write!(f, "\"meta\":{{\"name\":\"time_series\",\"fixed_length\":{}}},", 
                      if options.fixed_length { "true" } else { "false" });
        
        // 添加各个时间粒度的数据
        let _ = write!(f, "\"data\":{{");
        
        // 秒级数据
        self.describe_series_data(f, "second", &second_points);
        let _ = write!(f, ",");
        
        // 分钟级数据
        self.describe_series_data(f, "minute", &minute_points);
        let _ = write!(f, ",");
        
        // 小时级数据
        self.describe_series_data(f, "hour", &hour_points);
        let _ = write!(f, ",");
        
        // 天级数据
        self.describe_series_data(f, "day", &day_points);
        
        // 结束JSON对象
        let _ = write!(f, "}}}}");
    }
    
    /// 描述单个时间序列的数据
    fn describe_series_data(&self, f: &mut dyn fmt::Write, name: &str, points: &[DataPoint<T>]) {
        let _ = write!(f, "\"{}\":{{\"timestamps\":[", name);
        
        // 添加时间戳
        let mut first = true;
        for point in points {
            if !first {
                let _ = write!(f, ",");
            }
            first = false;
            let _ = write!(f, "{}", point.timestamp);
        }
        
        let _ = write!(f, "],\"values\":[");
        
        // 添加值
        first = true;
        for point in points {
            if !first {
                let _ = write!(f, ",");
            }
            first = false;
            let _ = write!(f, "{:?}", point.value);
        }
        
        let _ = write!(f, "]}}");
    }
}

/// 用于在控制台输出的序列格式化器
pub struct SeriesFormatter<'a, T, Op> {
    /// 序列
    series: &'a Series<T, Op>,
    /// 格式化选项
    options: SeriesOptions,
}

impl<'a, T, Op> SeriesFormatter<'a, T, Op>
where
    T: Clone + fmt::Debug + Send + Sync + 'static,
    Op: Combiner<T> + Clone + Send + Sync + 'static,
{
    /// 创建新的格式化器
    pub fn new(series: &'a Series<T, Op>, options: SeriesOptions) -> Self {
        Self { series, options }
    }
}

impl<'a, T, Op> fmt::Display for SeriesFormatter<'a, T, Op>
where
    T: Clone + fmt::Debug + Send + Sync + 'static,
    Op: Combiner<T> + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = String::new();
        self.series.describe(&mut buf, &self.options);
        write!(f, "{}", buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable::SeriesOptions;
    use crate::reducer::AddTo;

    #[test]
    fn test_series() {
        let series = Series::new(AddTo::default());
        series.append(1);

        /// sleep 1秒
        std::thread::sleep(Duration::from_secs(1));
        series.append(2);
        series.append(3);
        let formatter = SeriesFormatter::new(&series, SeriesOptions::default());
        println!("{}", formatter);

        let mut buf = String::new();
        series.describe(&mut buf, &SeriesOptions::default());
        println!("{}", buf);
        
    }
}