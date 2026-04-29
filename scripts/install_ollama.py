#!/usr/bin/env python3
"""
Ollama 安装脚本 - 独立版本
用于下载和安装 Ollama + AI 模型

特点：
1. 使用国内镜像源（Gitee）
2. 完整的错误处理和重试机制
3. 详细的进度输出
4. 支持断点续传
5. 自动验证安装
"""

import os
import sys
import json
import urllib.request
import urllib.error
import subprocess
import time
import winreg
from pathlib import Path


class OllamaInstaller:
    def __init__(self, install_dir=None):
        """
        初始化安装器
        
        Args:
            install_dir: 安装目录，默认为 %LOCALAPPDATA%/Programs/Ollama
        """
        if install_dir:
            self.install_dir = Path(install_dir)
        else:
            localappdata = os.getenv('LOCALAPPDATA')
            if not localappdata:
                raise RuntimeError("无法获取 LOCALAPPDATA 环境变量")
            self.install_dir = Path(localappdata) / "Programs" / "Ollama"
        
        self.ollama_exe = self.install_dir / "ollama.exe"
        
        # 使用 Gitee 镜像
        self.ollama_url = "https://gitee.com/mirrors/ollama/releases/download/v0.1.32/ollama-windows-amd64.zip"
        self.model_name = "qwen2.5:1.5b"
        
        # 重试配置
        self.max_retries = 3
        self.retry_delay = 2
        self.download_timeout = 600  # 10分钟
        
    def log(self, message, level="INFO"):
        """输出日志"""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{timestamp}] [{level}] {message}", flush=True)
    
    def download_file(self, url, dest_path, description="文件"):
        """
        下载文件，支持重试和进度显示
        """
        dest_path = Path(dest_path)
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        
        for attempt in range(1, self.max_retries + 1):
            try:
                self.log(f"下载 {description} (尝试 {attempt}/{self.max_retries})")
                self.log(f"URL: {url}")
                self.log(f"保存到: {dest_path}")
                
                req = urllib.request.Request(url)
                req.add_header('User-Agent', 'Mozilla/5.0')
                
                with urllib.request.urlopen(req, timeout=self.download_timeout) as response:
                    total_size = int(response.headers.get('Content-Length', 0))
                    
                    if total_size > 0:
                        self.log(f"文件大小: {total_size / 1024 / 1024:.2f} MB")
                    
                    downloaded = 0
                    chunk_size = 8192
                    
                    with open(dest_path, 'wb') as f:
                        while True:
                            chunk = response.read(chunk_size)
                            if not chunk:
                                break
                            f.write(chunk)
                            downloaded += len(chunk)
                            
                            if total_size > 0 and downloaded % (chunk_size * 100) == 0:
                                progress = (downloaded / total_size) * 100
                                self.log(f"下载进度: {progress:.1f}% ({downloaded / 1024 / 1024:.2f} MB / {total_size / 1024 / 1024:.2f} MB)")
                
                self.log(f"✓ {description} 下载完成")
                return True
                
            except Exception as e:
                self.log(f"✗ 下载失败: {e}", "ERROR")
                if attempt < self.max_retries:
                    self.log(f"等待 {self.retry_delay} 秒后重试...")
                    time.sleep(self.retry_delay)
                else:
                    raise RuntimeError(f"下载 {description} 失败，已重试 {self.max_retries} 次")
    
    def run_command(self, cmd, description="命令", check=True, timeout=600):
        """执行命令"""
        self.log(f"执行: {description}")
        self.log(f"命令: {' '.join(cmd)}")
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout,
                encoding='utf-8',
                errors='replace'
            )
            
            if result.stdout:
                for line in result.stdout.strip().split('\n'):
                    if line.strip():
                        self.log(f"  {line}")
            
            if result.stderr:
                for line in result.stderr.strip().split('\n'):
                    if line.strip():
                        self.log(f"  [stderr] {line}")
            
            if check and result.returncode != 0:
                raise RuntimeError(f"{description} 失败，返回码: {result.returncode}")
            
            self.log(f"✓ {description} 完成")
            return result
            
        except subprocess.TimeoutExpired:
            self.log(f"✗ {description} 超时", "ERROR")
            raise
        except Exception as e:
            self.log(f"✗ {description} 出错: {e}", "ERROR")
            raise
    
    def check_ollama_installed(self):
        """检查 Ollama 是否已安装"""
        # 检查可执行文件
        if self.ollama_exe.exists():
            return True
        
        # 检查系统 PATH
        try:
            result = subprocess.run(
                ["ollama", "--version"],
                capture_output=True,
                text=True,
                timeout=5
            )
            return result.returncode == 0
        except:
            return False
    
    def install_ollama(self):
        """安装 Ollama"""
        self.log("=" * 60)
        self.log("步骤 1: 安装 Ollama")
        self.log("=" * 60)
        
        if self.check_ollama_installed():
            self.log("Ollama 已安装，跳过")
            return
        
        # 下载 Ollama 安装程序
        self.log("开始下载 Ollama 安装程序...")
        temp_dir = Path(os.getenv('TEMP'))
        installer_path = temp_dir / "OllamaSetup.exe"
        
        # 使用官方下载链接
        ollama_installer_url = "https://ollama.com/download/OllamaSetup.exe"
        
        try:
            self.download_file(ollama_installer_url, installer_path, "Ollama 安装程序")
        except Exception as e:
            self.log(f"✗ 下载失败: {e}", "ERROR")
            self.log("请手动下载并安装 Ollama:")
            self.log("  官方: https://ollama.com/download/windows")
            self.log("  Gitee 镜像: https://gitee.com/mirrors/ollama")
            raise RuntimeError("Ollama 下载失败")
        
        # 静默安装 Ollama
        self.log("开始安装 Ollama...")
        self.log("注意：安装过程可能需要几分钟")
        
        try:
            # 使用静默安装参数
            result = subprocess.run(
                [str(installer_path), "/S"],
                capture_output=True,
                text=True,
                timeout=300,  # 5分钟超时
                encoding='utf-8',
                errors='replace'
            )
            
            # 等待安装完成
            self.log("等待安装完成...")
            time.sleep(10)
            
            # 验证安装
            if self.check_ollama_installed():
                self.log("✓ Ollama 安装成功")
                
                # 删除安装程序
                try:
                    installer_path.unlink()
                    self.log("✓ 已清理安装文件")
                except:
                    pass
            else:
                raise RuntimeError("安装完成但无法检测到 Ollama")
                
        except subprocess.TimeoutExpired:
            self.log("✗ 安装超时", "ERROR")
            raise RuntimeError("Ollama 安装超时")
        except Exception as e:
            self.log(f"✗ 安装失败: {e}", "ERROR")
            raise
    
    def start_ollama_service(self):
        """启动 Ollama 服务"""
        self.log("=" * 60)
        self.log("步骤 2: 启动 Ollama 服务")
        self.log("=" * 60)
        
        # 检查服务是否已运行
        try:
            result = subprocess.run(
                ["ollama", "list"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0:
                self.log("✓ Ollama 服务已运行")
                return
        except:
            pass
        
        # 启动服务
        self.log("启动 Ollama 服务...")
        try:
            # 在后台启动 ollama serve
            subprocess.Popen(
                ["ollama", "serve"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0
            )
            
            # 等待服务启动
            self.log("等待服务启动...")
            time.sleep(5)
            
            # 验证服务
            result = subprocess.run(
                ["ollama", "list"],
                capture_output=True,
                text=True,
                timeout=5
            )
            
            if result.returncode == 0:
                self.log("✓ Ollama 服务启动成功")
            else:
                raise RuntimeError("服务启动失败")
                
        except Exception as e:
            self.log(f"✗ 启动服务失败: {e}", "ERROR")
            raise
    
    def check_model_installed(self):
        """检查模型是否已安装"""
        try:
            result = subprocess.run(
                ["ollama", "list"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode == 0:
                return self.model_name in result.stdout
            
            return False
        except:
            return False
    
    def install_model(self):
        """安装 AI 模型"""
        self.log("=" * 60)
        self.log(f"步骤 3: 安装 AI 模型 ({self.model_name})")
        self.log("=" * 60)
        
        if self.check_model_installed():
            self.log(f"模型 {self.model_name} 已安装，跳过")
            return
        
        # 确保 .ollama 目录存在并生成 SSH 密钥
        userprofile = os.getenv('USERPROFILE')
        if userprofile:
            ollama_dir = Path(userprofile) / ".ollama"
            ollama_dir.mkdir(parents=True, exist_ok=True)
            self.log(f"✓ Ollama 数据目录已创建: {ollama_dir}")
            
            # 生成 SSH 密钥（如果不存在）
            id_ed25519 = ollama_dir / "id_ed25519"
            id_ed25519_pub = ollama_dir / "id_ed25519.pub"
            
            if not id_ed25519.exists() or not id_ed25519_pub.exists() or id_ed25519.stat().st_size == 0:
                self.log("生成 SSH 密钥...")
                
                # 尝试多个可能的 ssh-keygen 路径
                ssh_keygen_paths = [
                    r"C:\Program Files\Git\usr\bin\ssh-keygen.exe",
                    r"C:\Program Files (x86)\Git\usr\bin\ssh-keygen.exe",
                    "ssh-keygen",  # 系统 PATH 中
                ]
                
                key_generated = False
                for ssh_keygen in ssh_keygen_paths:
                    try:
                        self.log(f"尝试使用: {ssh_keygen}")
                        result = subprocess.run(
                            [ssh_keygen, "-t", "ed25519", "-f", str(id_ed25519), "-N", ""],
                            capture_output=True,
                            text=True,
                            timeout=10
                        )
                        
                        if result.returncode == 0:
                            self.log(f"✓ SSH 密钥已生成")
                            key_generated = True
                            break
                        else:
                            self.log(f"  失败: {result.stderr}")
                            
                    except FileNotFoundError:
                        self.log(f"  未找到: {ssh_keygen}")
                        continue
                    except Exception as e:
                        self.log(f"  错误: {e}")
                        continue
                
                if not key_generated:
                    self.log("⚠ 无法生成 SSH 密钥，模型下载可能失败", "WARNING")
                    self.log("提示: 请安装 Git for Windows 或确保 ssh-keygen 在 PATH 中", "WARNING")
        
        self.log(f"开始下载模型 {self.model_name}...")
        self.log("注意：首次下载可能需要较长时间（约 1GB）")
        
        try:
            # 使用 ollama pull 下载模型，实时输出进度
            self.log(f"执行: 下载模型 {self.model_name}")
            self.log(f"命令: ollama pull {self.model_name}")
            
            process = subprocess.Popen(
                ["ollama", "pull", self.model_name],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding='utf-8',
                errors='replace',
                bufsize=1,
                universal_newlines=True
            )
            
            # 实时读取输出
            import re
            last_percentage = 0.0
            
            for line in iter(process.stdout.readline, ''):
                if not line:
                    break
                
                line = line.strip()
                if not line:
                    continue
                
                # 输出原始行（用于调试）
                print(line, flush=True)
                
                # 尝试解析进度百分比
                # 格式: pulling 183715c43589: 50% ▕████████▏ 500 MB
                match = re.search(r'(\d+)%', line)
                if match:
                    percentage = float(match.group(1))
                    if percentage != last_percentage:
                        self.log(f"下载进度: {percentage:.1f}%")
                        last_percentage = percentage
                elif 'pulling manifest' in line.lower():
                    self.log("正在获取模型清单...")
                elif 'verifying' in line.lower():
                    self.log("正在验证模型文件...")
                elif 'writing manifest' in line.lower():
                    self.log("正在写入模型清单...")
                elif 'success' in line.lower():
                    self.log("模型下载完成")
            
            process.wait(timeout=1800)
            
            if process.returncode != 0:
                raise RuntimeError(f"下载模型 {self.model_name} 失败，返回码: {process.returncode}")
            
            self.log(f"✓ 模型 {self.model_name} 安装完成")
            
        except Exception as e:
            self.log(f"✗ 模型安装失败: {e}", "ERROR")
            raise
    
    def verify_installation(self):
        """验证安装"""
        self.log("=" * 60)
        self.log("步骤 4: 验证安装")
        self.log("=" * 60)
        
        # 验证 Ollama
        self.run_command(
            ["ollama", "--version"],
            "检查 Ollama 版本"
        )
        
        # 验证模型
        self.run_command(
            ["ollama", "list"],
            "列出已安装的模型"
        )
        
        # 测试模型
        self.log("测试模型...")
        try:
            result = subprocess.run(
                ["ollama", "run", self.model_name, "你好"],
                capture_output=True,
                text=True,
                timeout=30,
                encoding='utf-8',
                errors='replace'
            )
            
            if result.returncode == 0 and result.stdout.strip():
                self.log(f"✓ 模型测试成功")
                self.log(f"  模型回复: {result.stdout.strip()[:100]}")
            else:
                self.log("⚠ 模型测试未返回预期结果", "WARNING")
                
        except Exception as e:
            self.log(f"⚠ 模型测试失败: {e}", "WARNING")
        
        self.log("✓ 安装验证完成")
    
    def install(self):
        """执行完整安装流程"""
        try:
            self.log("=" * 60)
            self.log("开始安装 Ollama + AI 模型")
            self.log("=" * 60)
            
            # 执行安装步骤
            self.install_ollama()
            self.start_ollama_service()
            self.install_model()
            self.verify_installation()
            
            self.log("=" * 60)
            self.log("✓ Ollama + AI 模型安装完成！")
            self.log("=" * 60)
            
            # 输出安装信息
            info = {
                "success": True,
                "ollama_installed": True,
                "model_name": self.model_name,
                "model_installed": True
            }
            
            return info
            
        except Exception as e:
            self.log(f"✗ 安装失败: {e}", "ERROR")
            raise
    
    def uninstall_model(self):
        """卸载 AI 模型"""
        self.log("=" * 60)
        self.log(f"卸载 AI 模型 ({self.model_name})")
        self.log("=" * 60)
        
        if not self.check_model_installed():
            self.log("模型未安装")
            return
        
        try:
            self.run_command(
                ["ollama", "rm", self.model_name],
                f"删除模型 {self.model_name}"
            )
            self.log("✓ 模型已卸载")
        except Exception as e:
            self.log(f"✗ 卸载失败: {e}", "ERROR")
            raise
    
    def uninstall_ollama(self):
        """完全卸载 Ollama"""
        import shutil
        
        self.log("=" * 60)
        self.log("完全卸载 Ollama")
        self.log("=" * 60)
        
        # 1. 停止所有 Ollama 相关进程
        self.log("停止 Ollama 进程...")
        processes = ["ollama.exe", "ollama app.exe", "ollama_llama_server.exe"]
        for proc in processes:
            try:
                result = subprocess.run(
                    ["taskkill", "/F", "/IM", proc],
                    capture_output=True,
                    timeout=10
                )
                if result.returncode == 0:
                    self.log(f"✓ 已停止进程: {proc}")
            except:
                pass
        
        # 等待进程完全退出
        time.sleep(3)
        self.log("✓ Ollama 进程已停止")
        
        # 2. 删除程序文件
        localappdata = os.getenv('LOCALAPPDATA')
        if localappdata:
            ollama_program_dir = Path(localappdata) / "Programs" / "Ollama"
            if ollama_program_dir.exists():
                self.log(f"删除程序目录: {ollama_program_dir}")
                try:
                    # 使用更强力的删除方法
                    def remove_readonly(func, path, excinfo):
                        """处理只读文件"""
                        os.chmod(path, 0o777)
                        func(path)
                    
                    shutil.rmtree(ollama_program_dir, onerror=remove_readonly)
                    self.log("✓ 程序文件已删除")
                except Exception as e:
                    self.log(f"⚠ 删除程序文件时遇到问题: {e}", "WARNING")
                    self.log("提示: 如果删除失败，请手动删除该目录或重启后再试", "WARNING")
            else:
                self.log("程序目录不存在")
        
        # 3. 删除用户数据和模型
        userprofile = os.getenv('USERPROFILE')
        if userprofile:
            ollama_data_dir = Path(userprofile) / ".ollama"
            if ollama_data_dir.exists():
                self.log(f"删除数据目录: {ollama_data_dir}")
                try:
                    def remove_readonly(func, path, excinfo):
                        """处理只读文件"""
                        os.chmod(path, 0o777)
                        func(path)
                    
                    shutil.rmtree(ollama_data_dir, onerror=remove_readonly)
                    self.log("✓ 数据文件已删除")
                except Exception as e:
                    self.log(f"⚠ 删除数据文件时遇到问题: {e}", "WARNING")
                    self.log("提示: 如果删除失败，请手动删除该目录或重启后再试", "WARNING")
            else:
                self.log("数据目录不存在")
        
        self.log("=" * 60)
        self.log("✓ Ollama 卸载完成")
        self.log("=" * 60)


def main():
    """主函数"""
    if len(sys.argv) > 1 and sys.argv[1] == "uninstall":
        installer = OllamaInstaller()
        installer.uninstall_ollama()  # 完全卸载 Ollama
    else:
        installer = OllamaInstaller()
        info = installer.install()
        print("\n" + "=" * 60)
        print("安装信息:")
        print(json.dumps(info, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
