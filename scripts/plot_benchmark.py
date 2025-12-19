#!/usr/bin/env python3
"""
生成撮合引擎性能指标折线图
"""

import matplotlib.pyplot as plt
import pandas as pd
import numpy as np
import sys
import os

# 设置中文字体
plt.rcParams['font.sans-serif'] = ['Arial Unicode MS', 'SimHei', 'DejaVu Sans']
plt.rcParams['axes.unicode_minus'] = False

def plot_benchmark_results():
    """读取 CSV 数据并生成多指标折线图"""
    
    csv_file = 'benchmark_results.csv'
    if not os.path.exists(csv_file):
        print(f"错误: 找不到 {csv_file}")
        print("请先运行: cargo run --example generate_benchmark_data --release")
        sys.exit(1)
    
    # 读取数据
    try:
        df = pd.read_csv(csv_file)
    except Exception as e:
        print(f"读取 CSV 文件失败: {e}")
        sys.exit(1)
    
    # 创建图表
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))
    fig.suptitle('撮合引擎性能指标', fontsize=18, fontweight='bold', y=0.995)
    
    # 颜色方案
    colors = {
        'tps': '#2E86AB',      # 蓝色
        'qps': '#A23B72',      # 紫色
        'memory': '#F18F01',   # 橙色
        'duration': '#C73E1D'  # 红色
    }
    
    # 1. TPS 折线图
    ax1 = axes[0, 0]
    ax1.plot(df['Orders'], df['TPS'], marker='o', linewidth=2.5, 
             markersize=10, color=colors['tps'], label='TPS')
    ax1.set_xlabel('订单数量', fontsize=13, fontweight='bold')
    ax1.set_ylabel('TPS (订单/秒)', fontsize=13, fontweight='bold')
    ax1.set_title('吞吐量 (Transactions Per Second)', fontsize=14, fontweight='bold')
    ax1.grid(True, alpha=0.3, linestyle='--')
    ax1.set_xscale('log')
    ax1.legend(fontsize=11)
    
    # 添加数值标签
    for i, (x, y) in enumerate(zip(df['Orders'], df['TPS'])):
        if i % 2 == 0:  # 只标注部分点
            ax1.annotate(f'{y:.0f}', (x, y), textcoords="offset points", 
                        xytext=(0,10), ha='center', fontsize=9)
    
    # 2. QPS 折线图
    ax2 = axes[0, 1]
    ax2.plot(df['Orders'], df['QPS'], marker='s', linewidth=2.5, 
             markersize=10, color=colors['qps'], label='QPS')
    ax2.set_xlabel('订单数量', fontsize=13, fontweight='bold')
    ax2.set_ylabel('QPS (成交/秒)', fontsize=13, fontweight='bold')
    ax2.set_title('成交速率 (Queries Per Second)', fontsize=14, fontweight='bold')
    ax2.grid(True, alpha=0.3, linestyle='--')
    ax2.set_xscale('log')
    ax2.legend(fontsize=11)
    
    # 添加数值标签
    for i, (x, y) in enumerate(zip(df['Orders'], df['QPS'])):
        if i % 2 == 0:
            ax2.annotate(f'{y:.0f}', (x, y), textcoords="offset points", 
                        xytext=(0,10), ha='center', fontsize=9)
    
    # 3. 内存使用折线图
    ax3 = axes[1, 0]
    ax3.plot(df['Orders'], df['Memory_MB'], marker='^', linewidth=2.5, 
             markersize=10, color=colors['memory'], label='内存使用')
    ax3.set_xlabel('订单数量', fontsize=13, fontweight='bold')
    ax3.set_ylabel('内存使用 (MB)', fontsize=13, fontweight='bold')
    ax3.set_title('内存占用', fontsize=14, fontweight='bold')
    ax3.grid(True, alpha=0.3, linestyle='--')
    ax3.set_xscale('log')
    ax3.legend(fontsize=11)
    
    # 添加数值标签
    for i, (x, y) in enumerate(zip(df['Orders'], df['Memory_MB'])):
        if i % 2 == 0:
            ax3.annotate(f'{y:.1f}MB', (x, y), textcoords="offset points", 
                        xytext=(0,10), ha='center', fontsize=9)
    
    # 4. 延迟折线图
    ax4 = axes[1, 1]
    ax4.plot(df['Orders'], df['Duration_MS'], marker='d', linewidth=2.5, 
             markersize=10, color=colors['duration'], label='处理时间')
    ax4.set_xlabel('订单数量', fontsize=13, fontweight='bold')
    ax4.set_ylabel('处理时间 (毫秒)', fontsize=13, fontweight='bold')
    ax4.set_title('延迟', fontsize=14, fontweight='bold')
    ax4.grid(True, alpha=0.3, linestyle='--')
    ax4.set_xscale('log')
    ax4.legend(fontsize=11)
    
    # 添加数值标签
    for i, (x, y) in enumerate(zip(df['Orders'], df['Duration_MS'])):
        if i % 2 == 0:
            ax4.annotate(f'{y:.1f}ms', (x, y), textcoords="offset points", 
                        xytext=(0,10), ha='center', fontsize=9)
    
    # 调整布局
    plt.tight_layout(rect=[0, 0, 1, 0.98])
    
    # 保存图表
    output_file = 'benchmark_results.png'
    plt.savefig(output_file, dpi=300, bbox_inches='tight', facecolor='white')
    print(f'✓ 图表已保存到 {output_file}')
    
    # 显示统计信息
    print('\n=== 性能统计 ===')
    print(f"最大 TPS: {df['TPS'].max():.2f}")
    print(f"最大 QPS: {df['QPS'].max():.2f}")
    print(f"最大内存: {df['Memory_MB'].max():.2f} MB")
    print(f"最大延迟: {df['Duration_MS'].max():.2f} ms")

if __name__ == '__main__':
    plot_benchmark_results()

