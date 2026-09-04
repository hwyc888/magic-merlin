//! 智能路由模块 - 强化学习驱动的路径选择
//!
//! 原理：
//! 1. 使用 Q-Learning 算法学习最佳路径
//! 2. 根据延迟、丢包、带宽动态调整
//! 3. 自动避开被封锁的路径
//! 4. 支持多臂老虎机探索策略

use std::collections::HashMap;
use std::time::{Duration, Instant};
use rand::Rng;

/// 智能路由配置
#[derive(Clone, Debug)]
pub struct SmartRoutingConfig {
    /// 是否启用
    pub enabled: bool,
    /// 学习率
    pub learning_rate: f64,
    /// 折扣因子
    pub discount_factor: f64,
    /// 探索率 (epsilon)
    pub exploration_rate: f64,
    /// 探索率衰减
    pub exploration_decay: f64,
    /// 最小探索率
    pub min_exploration: f64,
    /// 奖励权重
    pub reward_weights: RewardWeights,
}

impl Default for SmartRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            learning_rate: 0.1,
            discount_factor: 0.95,
            exploration_rate: 0.3,
            exploration_decay: 0.995,
            min_exploration: 0.05,
            reward_weights: RewardWeights::default(),
        }
    }
}

/// 奖励权重
#[derive(Clone, Debug)]
pub struct RewardWeights {
    pub latency: f64,
    pub throughput: f64,
    pub stability: f64,
    pub success_rate: f64,
}

impl Default for RewardWeights {
    fn default() -> Self {
        Self {
            latency: 0.3,
            throughput: 0.3,
            stability: 0.2,
            success_rate: 0.2,
        }
    }
}

/// 路径状态
#[derive(Clone, Debug)]
pub struct PathMetrics {
    /// 路径 ID
    pub id: String,
    /// 协议类型
    pub protocol: String,
    /// 平均延迟 (ms)
    pub avg_latency: f64,
    /// 延迟方差
    pub latency_variance: f64,
    /// 吞吐量 (bytes/s)
    pub throughput: f64,
    /// 成功率
    pub success_rate: f64,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 是否被封锁
    pub blocked: bool,
    /// 上次使用时间
    pub last_used: Instant,
    /// 使用次数
    pub use_count: u64,
}

impl PathMetrics {
    pub fn new(id: String, protocol: String) -> Self {
        Self {
            id,
            protocol,
            avg_latency: 100.0,
            latency_variance: 0.0,
            throughput: 1_000_000.0,
            success_rate: 1.0,
            consecutive_failures: 0,
            blocked: false,
            last_used: Instant::now(),
            use_count: 0,
        }
    }

    /// 计算路径分数
    pub fn score(&self, weights: &RewardWeights) -> f64 {
        if self.blocked {
            return 0.0;
        }

        let latency_score = 1.0 / (self.avg_latency / 100.0 + 1.0);
        let throughput_score = (self.throughput / 1_000_000.0).min(10.0) / 10.0;
        let stability_score = 1.0 / (self.latency_variance / 100.0 + 1.0);
        let success_score = self.success_rate;

        weights.latency * latency_score
            + weights.throughput * throughput_score
            + weights.stability * stability_score
            + weights.success_rate * success_score
    }
}

/// Q-Learning 路由器
pub struct QLearningRouter {
    config: SmartRoutingConfig,
    /// Q 值表: (状态, 动作) -> Q值
    q_table: HashMap<(String, String), f64>,
    /// 路径指标
    paths: HashMap<String, PathMetrics>,
    /// 当前状态
    current_state: String,
    /// 当前探索率
    current_epsilon: f64,
    /// 总决策次数
    total_decisions: u64,
}

impl QLearningRouter {
    pub fn new(config: SmartRoutingConfig) -> Self {
        Self {
            current_epsilon: config.exploration_rate,
            config,
            q_table: HashMap::new(),
            paths: HashMap::new(),
            current_state: "normal".to_string(),
            total_decisions: 0,
        }
    }

    /// 添加路径
    pub fn add_path(&mut self, id: String, protocol: String) {
        self.paths.insert(id.clone(), PathMetrics::new(id, protocol));
    }

    /// 移除路径
    pub fn remove_path(&mut self, id: &str) {
        self.paths.remove(id);
    }

    /// 选择最佳路径
    pub fn select_path(&mut self) -> Option<String> {
        if self.paths.is_empty() {
            return None;
        }

        self.total_decisions += 1;
        let mut rng = rand::thread_rng();

        // Epsilon-greedy 策略
        let path_id = if rng.gen_bool(self.current_epsilon) {
            // 探索：随机选择
            self.random_path()
        } else {
            // 利用：选择 Q 值最高的
            self.best_path()
        };

        // 衰减探索率
        self.current_epsilon = (self.current_epsilon * self.config.exploration_decay)
            .max(self.config.min_exploration);

        // 更新使用统计
        if let Some(path) = self.paths.get_mut(&path_id) {
            path.last_used = Instant::now();
            path.use_count += 1;
        }

        Some(path_id)
    }

    /// 随机选择路径
    fn random_path(&self) -> String {
        let available: Vec<_> = self.paths.iter()
            .filter(|(_, p)| !p.blocked)
            .map(|(id, _)| id.clone())
            .collect();

        if available.is_empty() {
            return self.paths.keys().next().cloned().unwrap_or_default();
        }

        let mut rng = rand::thread_rng();
        available[rng.gen_range(0..available.len())].clone()
    }

    /// 选择最佳路径
    fn best_path(&self) -> String {
        let state = &self.current_state;
        
        self.paths.iter()
            .filter(|(_, p)| !p.blocked)
            .max_by(|(id_a, _), (id_b, _)| {
                let q_a = self.q_table.get(&(state.clone(), id_a.to_string())).unwrap_or(&0.0);
                let q_b = self.q_table.get(&(state.clone(), id_b.to_string())).unwrap_or(&0.0);
                q_a.partial_cmp(q_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| self.random_path())
    }

    /// 更新 Q 值
    pub fn update(&mut self, path_id: &str, reward: f64, next_state: &str) {
        let state = self.current_state.clone();
        let action = path_id.to_string();
        
        // 获取当前 Q 值
        let current_q = *self.q_table.get(&(state.clone(), action.clone())).unwrap_or(&0.0);
        
        // 获取下一状态的最大 Q 值
        let max_next_q = self.paths.keys()
            .filter_map(|a| self.q_table.get(&(next_state.to_string(), a.clone())))
            .fold(0.0f64, |a, &b| a.max(b));
        
        // Q-Learning 更新公式
        let new_q = current_q + self.config.learning_rate 
            * (reward + self.config.discount_factor * max_next_q - current_q);
        
        self.q_table.insert((state, action), new_q);
        self.current_state = next_state.to_string();
    }

    /// 报告路径结果
    pub fn report_result(&mut self, path_id: &str, latency: Duration, bytes: u64, success: bool) {
        let latency_ms = latency.as_millis() as f64;
        
        // 先计算需要的值
        let (reward, next_state, should_update) = if let Some(path) = self.paths.get_mut(path_id) {
            // 更新延迟（指数移动平均）
            let alpha = 0.2;
            let old_latency = path.avg_latency;
            path.avg_latency = alpha * latency_ms + (1.0 - alpha) * path.avg_latency;
            
            // 更新方差
            let diff = latency_ms - old_latency;
            path.latency_variance = alpha * diff * diff + (1.0 - alpha) * path.latency_variance;
            
            // 更新吞吐量
            if latency_ms > 0.0 {
                let throughput = bytes as f64 / (latency_ms / 1000.0);
                path.throughput = alpha * throughput + (1.0 - alpha) * path.throughput;
            }
            
            // 更新成功率
            if success {
                path.success_rate = alpha * 1.0 + (1.0 - alpha) * path.success_rate;
                path.consecutive_failures = 0;
            } else {
                path.success_rate = alpha * 0.0 + (1.0 - alpha) * path.success_rate;
                path.consecutive_failures += 1;
                
                // 连续失败太多次，标记为封锁
                if path.consecutive_failures >= 5 {
                    path.blocked = true;
                    tracing::warn!("路径 {} 被标记为封锁", path_id);
                }
            }
            
            // 计算奖励
            let reward = if path.blocked {
                -10.0
            } else {
                path.score(&self.config.reward_weights) * 10.0 - 5.0
            };
            
            // 确定下一状态
            let next_state = if path.blocked {
                "blocked"
            } else if path.success_rate < 0.5 {
                "degraded"
            } else {
                "normal"
            };
            
            (reward, next_state.to_string(), true)
        } else {
            (0.0, "normal".to_string(), false)
        };
        
        if should_update {
            self.update(path_id, reward, &next_state);
        }
    }

    /// 计算奖励
    fn calculate_reward(&self, path: &PathMetrics) -> f64 {
        if path.blocked {
            return -10.0;
        }
        
        path.score(&self.config.reward_weights) * 10.0 - 5.0
    }

    /// 重置封锁状态（定期尝试）
    pub fn reset_blocked_paths(&mut self) {
        for path in self.paths.values_mut() {
            if path.blocked && path.last_used.elapsed() > Duration::from_secs(300) {
                path.blocked = false;
                path.consecutive_failures = 0;
                tracing::info!("路径 {} 封锁状态已重置", path.id);
            }
        }
    }

    /// 获取路径统计
    pub fn get_stats(&self) -> RouterStats {
        RouterStats {
            total_decisions: self.total_decisions,
            current_epsilon: self.current_epsilon,
            current_state: self.current_state.clone(),
            path_count: self.paths.len(),
            blocked_count: self.paths.values().filter(|p| p.blocked).count(),
            q_table_size: self.q_table.len(),
        }
    }
}

/// 路由器统计
#[derive(Clone, Debug)]
pub struct RouterStats {
    pub total_decisions: u64,
    pub current_epsilon: f64,
    pub current_state: String,
    pub path_count: usize,
    pub blocked_count: usize,
    pub q_table_size: usize,
}

/// 多臂老虎机路由器 (UCB1 算法)
pub struct BanditRouter {
    /// 路径臂
    arms: HashMap<String, BanditArm>,
    /// 总拉取次数
    total_pulls: u64,
    /// UCB 探索参数
    exploration_param: f64,
}

/// 老虎机臂
#[derive(Clone, Debug)]
struct BanditArm {
    /// 拉取次数
    pulls: u64,
    /// 累计奖励
    total_reward: f64,
    /// 平均奖励
    avg_reward: f64,
}

impl BanditRouter {
    pub fn new(exploration_param: f64) -> Self {
        Self {
            arms: HashMap::new(),
            total_pulls: 0,
            exploration_param,
        }
    }

    /// 添加臂
    pub fn add_arm(&mut self, id: String) {
        self.arms.insert(id, BanditArm {
            pulls: 0,
            total_reward: 0.0,
            avg_reward: 0.0,
        });
    }

    /// UCB1 选择
    pub fn select(&mut self) -> Option<String> {
        if self.arms.is_empty() {
            return None;
        }

        // 确保每个臂至少被拉一次
        for (id, arm) in &self.arms {
            if arm.pulls == 0 {
                return Some(id.clone());
            }
        }

        // UCB1 公式选择
        let ln_total = (self.total_pulls as f64).ln();
        
        self.arms.iter()
            .max_by(|(_, a), (_, b)| {
                let ucb_a = a.avg_reward + self.exploration_param * (ln_total / a.pulls as f64).sqrt();
                let ucb_b = b.avg_reward + self.exploration_param * (ln_total / b.pulls as f64).sqrt();
                ucb_a.partial_cmp(&ucb_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id.clone())
    }

    /// 更新奖励
    pub fn update(&mut self, id: &str, reward: f64) {
        if let Some(arm) = self.arms.get_mut(id) {
            arm.pulls += 1;
            arm.total_reward += reward;
            arm.avg_reward = arm.total_reward / arm.pulls as f64;
            self.total_pulls += 1;
        }
    }
}

/// Thompson Sampling 路由器
pub struct ThompsonRouter {
    /// 路径的 Beta 分布参数
    arms: HashMap<String, (f64, f64)>, // (alpha, beta)
}

impl ThompsonRouter {
    pub fn new() -> Self {
        Self {
            arms: HashMap::new(),
        }
    }

    /// 添加臂
    pub fn add_arm(&mut self, id: String) {
        self.arms.insert(id, (1.0, 1.0)); // 均匀先验
    }

    /// Thompson Sampling 选择
    pub fn select(&self) -> Option<String> {
        use rand_distr::{Beta, Distribution};
        let mut rng = rand::thread_rng();

        self.arms.iter()
            .max_by(|(_, (a1, b1)), (_, (a2, b2))| {
                let sample1 = Beta::new(*a1, *b1).unwrap().sample(&mut rng);
                let sample2 = Beta::new(*a2, *b2).unwrap().sample(&mut rng);
                sample1.partial_cmp(&sample2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id.clone())
    }

    /// 更新（伯努利奖励）
    pub fn update(&mut self, id: &str, success: bool) {
        if let Some((alpha, beta)) = self.arms.get_mut(id) {
            if success {
                *alpha += 1.0;
            } else {
                *beta += 1.0;
            }
        }
    }
}

impl Default for ThompsonRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q_learning_router() {
        let config = SmartRoutingConfig::default();
        let mut router = QLearningRouter::new(config);

        router.add_path("path1".to_string(), "tcp".to_string());
        router.add_path("path2".to_string(), "udp".to_string());

        // 选择路径
        let selected = router.select_path();
        assert!(selected.is_some());

        // 报告结果
        router.report_result(
            &selected.unwrap(),
            Duration::from_millis(50),
            1000,
            true,
        );
    }

    #[test]
    fn test_bandit_router() {
        let mut router = BanditRouter::new(2.0);

        router.add_arm("arm1".to_string());
        router.add_arm("arm2".to_string());

        // 初始选择应该遍历所有臂
        let s1 = router.select().unwrap();
        router.update(&s1, 1.0);

        let s2 = router.select().unwrap();
        router.update(&s2, 0.5);

        // 之后应该倾向于选择奖励高的
        for _ in 0..10 {
            let s = router.select().unwrap();
            router.update(&s, if s == "arm1" { 1.0 } else { 0.5 });
        }
    }

    #[test]
    fn test_path_metrics() {
        let path = PathMetrics::new("test".to_string(), "tcp".to_string());
        let weights = RewardWeights::default();
        
        let score = path.score(&weights);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }
}
