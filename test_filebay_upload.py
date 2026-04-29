#!/usr/bin/env python3
"""
FileBay 上传功能测试脚本
测试文件上传到 FileBay 的完整流程
"""

import requests
import json
import base64
import os
import sys
from pathlib import Path
import urllib3

# 禁用 SSL 警告（仅用于测试）
urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

# FileBay 配置
CONFIG_FILE = "filebay-config.json"

def load_config():
    """加载 FileBay 配置"""
    if not os.path.exists(CONFIG_FILE):
        print(f"❌ 配置文件不存在: {CONFIG_FILE}")
        sys.exit(1)
    
    with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    print("✓ 配置已加载:")
    print(f"  URL: {config['url']}")
    print(f"  用户: {config['username']}")
    print(f"  仓库: {config['repoName']}")
    print(f"  Token: {config['token'][:10]}...")
    
    return config

def test_connection(config):
    """测试连接"""
    print("\n=== 测试 1: 连接测试 ===")
    
    url = f"{config['url']}/api/v1/user"
    headers = {
        "Authorization": f"token {config['token']}",
        "Content-Type": "application/json"
    }
    
    try:
        # 尝试不验证 SSL（用于测试环境）
        response = requests.get(url, headers=headers, timeout=10, verify=False)
        print(f"状态码: {response.status_code}")
        
        if response.status_code == 200:
            user_info = response.json()
            print(f"✓ 连接成功！")
            print(f"  用户名: {user_info.get('login', 'N/A')}")
            print(f"  邮箱: {user_info.get('email', 'N/A')}")
            return True
        elif response.status_code == 401:
            print(f"❌ 认证失败: Token 无效")
            print(f"响应: {response.text}")
            return False
        else:
            print(f"❌ 连接失败: HTTP {response.status_code}")
            print(f"响应: {response.text}")
            return False
    except requests.exceptions.SSLError as e:
        print(f"❌ SSL 错误: {e}")
        print("提示: 这可能是测试环境的证书问题")
        return False
    except requests.exceptions.ConnectionError as e:
        print(f"❌ 连接错误: {e}")
        return False
    except Exception as e:
        print(f"❌ 连接异常: {e}")
        return False

def check_repo(config):
    """检查仓库是否存在"""
    print("\n=== 测试 2: 检查仓库 ===")
    
    url = f"{config['url']}/api/v1/repos/{config['username']}/{config['repoName']}"
    headers = {
        "Authorization": f"token {config['token']}",
        "Content-Type": "application/json"
    }
    
    try:
        response = requests.get(url, headers=headers, timeout=10, verify=False)
        print(f"状态码: {response.status_code}")
        
        if response.status_code == 200:
            repo_info = response.json()
            print(f"✓ 仓库存在！")
            print(f"  名称: {repo_info.get('name', 'N/A')}")
            print(f"  描述: {repo_info.get('description', 'N/A')}")
            print(f"  私有: {repo_info.get('private', False)}")
            return True
        elif response.status_code == 404:
            print(f"❌ 仓库不存在")
            return False
        else:
            print(f"❌ 检查失败: HTTP {response.status_code}")
            print(f"响应: {response.text}")
            return False
    except Exception as e:
        print(f"❌ 检查异常: {e}")
        return False

def create_test_file():
    """创建测试文件"""
    print("\n=== 准备测试文件 ===")
    
    test_file = "test_upload.txt"
    content = f"""这是一个测试文件
用于测试 FileBay 上传功能
时间: {os.popen('date /t & time /t').read().strip()}
"""
    
    with open(test_file, 'w', encoding='utf-8') as f:
        f.write(content)
    
    print(f"✓ 测试文件已创建: {test_file}")
    print(f"  内容: {len(content)} 字节")
    
    return test_file

def upload_file(config, local_file, remote_path):
    """上传文件到 FileBay"""
    print(f"\n=== 测试 3: 上传文件 ===")
    print(f"本地文件: {local_file}")
    print(f"远程路径: {remote_path}")
    
    # 读取文件内容
    with open(local_file, 'rb') as f:
        content = f.read()
    
    # Base64 编码
    content_b64 = base64.b64encode(content).decode('utf-8')
    print(f"文件大小: {len(content)} 字节")
    print(f"Base64 大小: {len(content_b64)} 字节")
    
    # 检查文件是否已存在
    url = f"{config['url']}/api/v1/repos/{config['username']}/{config['repoName']}/contents/{remote_path}"
    headers = {
        "Authorization": f"token {config['token']}",
        "Content-Type": "application/json"
    }
    
    sha = None
    try:
        response = requests.get(url, headers=headers, timeout=10, verify=False)
        if response.status_code == 200:
            file_info = response.json()
            sha = file_info.get('sha')
            print(f"✓ 文件已存在，SHA: {sha}")
    except:
        pass
    
    # 上传文件
    payload = {
        "content": content_b64,
        "message": f"Test upload: {remote_path}"
    }
    
    if sha:
        payload["sha"] = sha
        print("执行更新操作...")
    else:
        print("执行创建操作...")
    
    try:
        # 使用 POST 方法（Gitea API）
        response = requests.post(url, headers=headers, json=payload, timeout=30, verify=False)
        print(f"状态码: {response.status_code}")
        
        if response.status_code in [200, 201]:
            result = response.json()
            print(f"✓ 上传成功！")
            
            # 获取文件信息
            if 'content' in result:
                file_url = result['content'].get('html_url', 'N/A')
                download_url = result['content'].get('download_url', 'N/A')
                print(f"  文件 URL: {file_url}")
                print(f"  下载 URL: {download_url}")
            
            return True
        else:
            print(f"❌ 上传失败: HTTP {response.status_code}")
            print(f"响应: {response.text}")
            return False
    except Exception as e:
        print(f"❌ 上传异常: {e}")
        return False

def verify_upload(config, remote_path):
    """验证上传的文件"""
    print(f"\n=== 测试 4: 验证上传 ===")
    
    url = f"{config['url']}/api/v1/repos/{config['username']}/{config['repoName']}/contents/{remote_path}"
    headers = {
        "Authorization": f"token {config['token']}",
        "Content-Type": "application/json"
    }
    
    try:
        response = requests.get(url, headers=headers, timeout=10, verify=False)
        print(f"状态码: {response.status_code}")
        
        if response.status_code == 200:
            file_info = response.json()
            print(f"✓ 文件验证成功！")
            print(f"  名称: {file_info.get('name', 'N/A')}")
            print(f"  大小: {file_info.get('size', 0)} 字节")
            print(f"  SHA: {file_info.get('sha', 'N/A')}")
            
            # 下载并验证内容
            download_url = file_info.get('download_url')
            if download_url:
                print(f"\n验证文件内容...")
                content_response = requests.get(download_url, headers=headers, timeout=10, verify=False)
                if content_response.status_code == 200:
                    content = content_response.text
                    print(f"✓ 文件内容:")
                    print("  " + "\n  ".join(content.split('\n')[:5]))
                    if len(content.split('\n')) > 5:
                        print("  ...")
            
            return True
        else:
            print(f"❌ 验证失败: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ 验证异常: {e}")
        return False

def cleanup(test_file):
    """清理测试文件"""
    print(f"\n=== 清理 ===")
    try:
        if os.path.exists(test_file):
            os.remove(test_file)
            print(f"✓ 已删除测试文件: {test_file}")
    except Exception as e:
        print(f"⚠ 清理失败: {e}")

def main():
    """主函数"""
    print("=" * 60)
    print("FileBay 上传功能测试")
    print("=" * 60)
    
    # 加载配置
    config = load_config()
    
    # 测试连接
    if not test_connection(config):
        print("\n❌ 连接测试失败，终止测试")
        sys.exit(1)
    
    # 检查仓库
    if not check_repo(config):
        print("\n❌ 仓库不存在，终止测试")
        sys.exit(1)
    
    # 创建测试文件
    test_file = create_test_file()
    
    try:
        # 上传文件
        remote_path = "test/test_upload.txt"
        if upload_file(config, test_file, remote_path):
            # 验证上传
            verify_upload(config, remote_path)
        else:
            print("\n❌ 上传失败")
    finally:
        # 清理
        cleanup(test_file)
    
    print("\n" + "=" * 60)
    print("测试完成")
    print("=" * 60)

if __name__ == "__main__":
    main()
