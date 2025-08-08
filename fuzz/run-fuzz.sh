#!/bin/bash

# Fuzzing script for PMCP
# Requires nightly Rust toolchain

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "==========================================="
echo "PMCP Fuzzing Infrastructure"
echo "==========================================="
echo ""

# Check if nightly toolchain is installed
if ! rustup toolchain list | grep -q nightly; then
    echo -e "${YELLOW}Installing nightly toolchain...${NC}"
    rustup install nightly
fi

# Function to run a single fuzz target
run_fuzz_target() {
    local target=$1
    local duration=${2:-60}
    
    echo -e "${GREEN}Running fuzz target: $target${NC}"
    echo "Duration: ${duration} seconds"
    
    rustup run nightly cargo fuzz run "$target" -- \
        -max_total_time="$duration" \
        -print_final_stats=1 \
        -detect_leaks=0 \
        2>&1 | tee "fuzz_${target}.log"
    
    echo ""
}

# Function to run all fuzz targets
run_all_targets() {
    local duration=${1:-60}
    
    for target in protocol_parsing jsonrpc_handling transport_layer auth_flows; do
        run_fuzz_target "$target" "$duration"
    done
}

# Function to run continuous fuzzing
run_continuous() {
    local target=$1
    
    echo -e "${GREEN}Starting continuous fuzzing for: $target${NC}"
    echo "Press Ctrl+C to stop"
    
    rustup run nightly cargo fuzz run "$target" -- \
        -print_final_stats=1 \
        -detect_leaks=0
}

# Function to minimize corpus
minimize_corpus() {
    local target=$1
    
    echo -e "${GREEN}Minimizing corpus for: $target${NC}"
    
    rustup run nightly cargo fuzz cmin "$target"
}

# Function to show coverage
show_coverage() {
    local target=$1
    
    echo -e "${GREEN}Generating coverage for: $target${NC}"
    
    rustup run nightly cargo fuzz coverage "$target"
    
    # Generate HTML report
    if command -v llvm-cov &> /dev/null; then
        llvm-cov show \
            "fuzz/target/x86_64-unknown-linux-gnu/release/$target" \
            -instr-profile="fuzz/coverage/$target/coverage.profdata" \
            -format=html \
            -output-dir="fuzz/coverage/$target/html" \
            -Xdemangler=rustfilt
        
        echo "Coverage report generated at: fuzz/coverage/$target/html/index.html"
    else
        echo -e "${YELLOW}llvm-cov not found. Install it for HTML coverage reports.${NC}"
    fi
}

# Main script
case "${1:-help}" in
    all)
        run_all_targets "${2:-60}"
        ;;
    continuous)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Target name required${NC}"
            echo "Usage: $0 continuous <target>"
            exit 1
        fi
        run_continuous "$2"
        ;;
    minimize)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Target name required${NC}"
            echo "Usage: $0 minimize <target>"
            exit 1
        fi
        minimize_corpus "$2"
        ;;
    coverage)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Target name required${NC}"
            echo "Usage: $0 coverage <target>"
            exit 1
        fi
        show_coverage "$2"
        ;;
    list)
        echo "Available fuzz targets:"
        echo "  - protocol_parsing"
        echo "  - jsonrpc_handling"
        echo "  - transport_layer"
        echo "  - auth_flows"
        ;;
    ci)
        # CI mode: run each target for 5 minutes
        echo -e "${GREEN}Running CI fuzzing (5 minutes per target)${NC}"
        run_all_targets 300
        ;;
    24h)
        # 24-hour fuzzing as specified in requirements
        echo -e "${GREEN}Starting 24-hour fuzzing run${NC}"
        run_all_targets 21600  # 6 hours per target = 24 hours total
        ;;
    help|*)
        echo "Usage: $0 <command> [options]"
        echo ""
        echo "Commands:"
        echo "  all [duration]        Run all fuzz targets (default: 60 seconds each)"
        echo "  continuous <target>   Run continuous fuzzing for a specific target"
        echo "  minimize <target>     Minimize the corpus for a target"
        echo "  coverage <target>     Generate coverage report for a target"
        echo "  list                  List all available fuzz targets"
        echo "  ci                    Run CI fuzzing (5 minutes per target)"
        echo "  24h                   Run 24-hour fuzzing (6 hours per target)"
        echo "  help                  Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 all 120                    # Run all targets for 2 minutes each"
        echo "  $0 continuous protocol_parsing # Continuous fuzzing for protocol parsing"
        echo "  $0 coverage jsonrpc_handling   # Generate coverage for JSON-RPC handling"
        echo ""
        if [ "$1" != "help" ]; then
            exit 1
        fi
        ;;
esac

echo -e "${GREEN}Fuzzing complete!${NC}"