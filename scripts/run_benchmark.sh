#!/bin/bash

# 运行基准测试并生成性能报告

echo "=== 运行撮合引擎基准测试 ==="

# 运行基准测试
cargo bench --bench comprehensive_bench 2>&1 | tee benchmark_output.txt

# 如果生成了 CSV 文件，尝试生成图表
if [ -f "benchmark_results.csv" ]; then
    echo "生成性能图表..."
    python3 plot_benchmark.py 2>/dev/null || echo "需要安装 matplotlib 和 pandas: pip3 install matplotlib pandas"
fi

echo "基准测试完成！"

