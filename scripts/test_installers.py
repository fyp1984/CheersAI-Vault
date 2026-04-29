#!/usr/bin/env python3
"""
测试安装脚本
"""

import sys
import subprocess
import json
from pathlib import Path


def test_ocr():
    """测试 OCR 安装"""
    print("=" * 60)
    print("测试 OCR 安装脚本")
    print("=" * 60)
    
    script_path = Path(__file__).parent / "install_ocr.py"
    
    try:
        # 运行安装脚本
        result = subprocess.run(
            [sys.executable, str(script_path)],
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace'
        )
        
        print("STDOUT:")
        print(result.stdout)
        
        if result.stderr:
            print("\nSTDERR:")
            print(result.stderr)
        
        if result.returncode == 0:
            print("\n✓ OCR 安装测试通过")
            
            # 尝试解析输出的 JSON
            try:
                lines = result.stdout.strip().split('\n')
                for line in reversed(lines):
                    if line.strip().startswith('{'):
                        info = json.loads(line)
                        print("\n安装信息:")
                        print(json.dumps(info, indent=2, ensure_ascii=False))
                        break
            except:
                pass
            
            return True
        else:
            print(f"\n✗ OCR 安装测试失败，返回码: {result.returncode}")
            return False
            
    except Exception as e:
        print(f"\n✗ 测试出错: {e}")
        return False


def test_ollama():
    """测试 Ollama 安装"""
    print("=" * 60)
    print("测试 Ollama 安装脚本")
    print("=" * 60)
    
    script_path = Path(__file__).parent / "install_ollama.py"
    
    try:
        # 运行安装脚本
        result = subprocess.run(
            [sys.executable, str(script_path)],
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace'
        )
        
        print("STDOUT:")
        print(result.stdout)
        
        if result.stderr:
            print("\nSTDERR:")
            print(result.stderr)
        
        if result.returncode == 0:
            print("\n✓ Ollama 安装测试通过")
            
            # 尝试解析输出的 JSON
            try:
                lines = result.stdout.strip().split('\n')
                for line in reversed(lines):
                    if line.strip().startswith('{'):
                        info = json.loads(line)
                        print("\n安装信息:")
                        print(json.dumps(info, indent=2, ensure_ascii=False))
                        break
            except:
                pass
            
            return True
        else:
            print(f"\n✗ Ollama 安装测试失败，返回码: {result.returncode}")
            return False
            
    except Exception as e:
        print(f"\n✗ 测试出错: {e}")
        return False


def main():
    """主函数"""
    if len(sys.argv) < 2:
        print("Usage: test_installers.py [ocr|ollama|all]")
        sys.exit(1)
    
    test_type = sys.argv[1].lower()
    
    results = {}
    
    if test_type in ['ocr', 'all']:
        results['ocr'] = test_ocr()
        print()
    
    if test_type in ['ollama', 'all']:
        results['ollama'] = test_ollama()
        print()
    
    # 输出总结
    print("=" * 60)
    print("测试总结")
    print("=" * 60)
    
    for name, passed in results.items():
        status = "✓ 通过" if passed else "✗ 失败"
        print(f"{name.upper()}: {status}")
    
    # 返回码
    if all(results.values()):
        print("\n✓ 所有测试通过")
        sys.exit(0)
    else:
        print("\n✗ 部分测试失败")
        sys.exit(1)


if __name__ == "__main__":
    main()
