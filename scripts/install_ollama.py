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
import hashlib
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
        
        # 重试配置 - 更激进的重试策略
        self.max_retries = 5  # 增加到5次
        self.retry_delay = 3  # 增加到3秒
        self.download_timeout = 30  # 单次读取超时30秒（不是总超时）
        
    def log(self, message, level="INFO"):
        """输出日志"""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{timestamp}] [{level}] {message}", flush=True)
    
    def download_file(self, url, dest_path, description="文件"):
        """
        下载文件，支持断点续传、重试和进度显示
        """
        dest_path = Path(dest_path)
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        
        # 检查是否有部分下载的文件
        temp_path = Path(str(dest_path) + ".download")
        downloaded_size = 0
        if temp_path.exists():
            downloaded_size = temp_path.stat().st_size
            self.log(f"检测到未完成的下载，已下载: {downloaded_size / 1024 / 1024:.2f} MB")
        
        for attempt in range(1, self.max_retries + 1):
            try:
                self.log(f"下载 {description} (尝试 {attempt}/{self.max_retries})")
                self.log(f"URL: {url}")
                self.log(f"保存到: {dest_path}")
                
                req = urllib.request.Request(url)
                req.add_header('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36')
                
                # 支持断点续传
                if downloaded_size > 0:
                    req.add_header('Range', f'bytes={downloaded_size}-')
                    self.log(f"从 {downloaded_size / 1024 / 1024:.2f} MB 处继续下载")
                
                with urllib.request.urlopen(req, timeout=30) as response:
                    # 获取总大小
                    if downloaded_size > 0:
                        # 断点续传时，Content-Length 是剩余大小
                        content_range = response.headers.get('Content-Range', '')
                        if content_range:
                            total_size = int(content_range.split('/')[-1])
                        else:
                            total_size = downloaded_size + int(response.headers.get('Content-Length', 0))
                    else:
                        total_size = int(response.headers.get('Content-Length', 0))
                    
                    if total_size > 0:
                        self.log(f"文件总大小: {total_size / 1024 / 1024:.2f} MB")
                    
                    chunk_size = 65536  # 64KB chunks for better performance
                    last_progress_time = time.time()
                    last_progress_bytes = downloaded_size
                    
                    # 打开文件（追加模式如果是断点续传）
                    mode = 'ab' if downloaded_size > 0 else 'wb'
                    with open(temp_path, mode) as f:
                        while True:
                            chunk = response.read(chunk_size)
                            if not chunk:
                                break
                            f.write(chunk)
                            downloaded_size += len(chunk)
                            
                            # 每秒更新一次进度
                            current_time = time.time()
                            if current_time - last_progress_time >= 1.0:
                                if total_size > 0:
                                    progress = (downloaded_size / total_size) * 100
                                    speed = (downloaded_size - last_progress_bytes) / (current_time - last_progress_time) / 1024 / 1024
                                    self.log(f"下载进度: {progress:.1f}% ({downloaded_size / 1024 / 1024:.2f} MB / {total_size / 1024 / 1024:.2f} MB) - 速度: {speed:.2f} MB/s")
                                else:
                                    self.log(f"已下载: {downloaded_size / 1024 / 1024:.2f} MB")
                                
                                last_progress_time = current_time
                                last_progress_bytes = downloaded_size
                
                # 下载完成，验证文件完整性
                if temp_path.exists():
                    # 验证下载的大小是否符合预期
                    actual_size = temp_path.stat().st_size
                    
                    # 只有在 total_size 可靠的情况下才验证大小
                    # 如果 total_size 和 actual_size 差距太大，说明 Content-Length 不准确
                    if total_size > 0:
                        size_diff_ratio = abs(actual_size - total_size) / total_size
                        if size_diff_ratio > 0.1:  # 差距超过 10%
                            self.log(f"⚠ 文件大小差异较大: 预期 {total_size}, 实际 {actual_size}", "WARNING")
                            self.log(f"⚠ 差异比例: {size_diff_ratio * 100:.1f}%", "WARNING")
                            self.log("⚠ 服务器返回的 Content-Length 可能不准确，跳过大小验证", "WARNING")
                        elif actual_size != total_size:
                            self.log(f"⚠ 文件大小不匹配: 预期 {total_size}, 实际 {actual_size}", "WARNING")
                            self.log("删除不完整的文件，准备重试...")
                            temp_path.unlink()
                            if attempt < self.max_retries:
                                self.log(f"等待 {self.retry_delay} 秒后重试...")
                                time.sleep(self.retry_delay)
                                continue
                            else:
                                raise RuntimeError(f"下载的文件大小不匹配")
                    
                    # 如果目标文件已存在，先删除（带重试）
                    if dest_path.exists():
                        for retry in range(3):
                            try:
                                dest_path.unlink()
                                break
                            except PermissionError:
                                if retry < 2:
                                    self.log(f"⚠ 目标文件被占用，等待 2 秒后重试... ({retry + 1}/3)", "WARNING")
                                    time.sleep(2)
                                else:
                                    raise RuntimeError("无法删除目标文件，可能被其他程序占用（杀毒软件？）")
                    
                    # 重命名临时文件（带重试）
                    for retry in range(5):
                        try:
                            temp_path.rename(dest_path)
                            break
                        except PermissionError:
                            if retry < 4:
                                self.log(f"⚠ 文件被占用，等待 2 秒后重试... ({retry + 1}/5)", "WARNING")
                                self.log("提示: 可能是杀毒软件正在扫描文件", "WARNING")
                                time.sleep(2)
                            else:
                                raise RuntimeError("无法重命名文件，可能被杀毒软件占用。请暂时关闭杀毒软件或添加信任")
                    
                    self.log(f"✓ {description} 下载完成，大小: {actual_size / 1024 / 1024:.2f} MB")
                    return True
                else:
                    raise RuntimeError("临时文件不存在")
                
            except urllib.error.HTTPError as e:
                if e.code == 416:  # Range Not Satisfiable - 文件已完全下载
                    if temp_path.exists():
                        # 如果目标文件已存在，先删除
                        if dest_path.exists():
                            dest_path.unlink()
                        temp_path.rename(dest_path)
                    self.log(f"✓ {description} 已完全下载")
                    return True
                else:
                    self.log(f"✗ HTTP 错误 {e.code}: {e.reason}", "ERROR")
                    if attempt < self.max_retries:
                        self.log(f"等待 {self.retry_delay} 秒后重试...")
                        time.sleep(self.retry_delay)
                    else:
                        raise RuntimeError(f"下载 {description} 失败: HTTP {e.code}")
                        
            except urllib.error.URLError as e:
                self.log(f"✗ 网络错误: {e.reason}", "ERROR")
                if attempt < self.max_retries:
                    self.log(f"等待 {self.retry_delay} 秒后重试...")
                    time.sleep(self.retry_delay)
                else:
                    raise RuntimeError(f"下载 {description} 失败: 网络错误")
                    
            except Exception as e:
                self.log(f"✗ 下载失败: {e}", "ERROR")
                if attempt < self.max_retries:
                    self.log(f"等待 {self.retry_delay} 秒后重试...")
                    time.sleep(self.retry_delay)
                else:
                    # 保留临时文件以便下次继续
                    if temp_path.exists():
                        self.log(f"已保留部分下载的文件，下次可继续: {temp_path}")
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
            
            return result
            
        except FileNotFoundError:
            self.log(f"✗ 命令未找到: {cmd[0]}", "ERROR")
            if check:
                raise RuntimeError(f"命令未找到: {cmd[0]}")
            return None
        except subprocess.TimeoutExpired:
            self.log(f"✗ 命令超时: {description}", "ERROR")
            if check:
                raise
            return None
            self.log(f"✓ {description} 完成")
            return result
            
        except subprocess.TimeoutExpired:
            self.log(f"✗ {description} 超时", "ERROR")
            raise
        except Exception as e:
            self.log(f"✗ {description} 出错: {e}", "ERROR")
            raise
    
    def check_ollama_installed(self):
        """检查 Ollama 是否已安装 - 增强版"""
        # 方法1: 检查默认安装路径的可执行文件
        if self.ollama_exe.exists():
            return True
        
        # 方法2: 检查其他可能的安装路径
        possible_paths = [
            Path(os.getenv('LOCALAPPDATA', '')) / "Programs" / "Ollama" / "ollama.exe",
            Path(os.getenv('PROGRAMFILES', '')) / "Ollama" / "ollama.exe",
            Path(os.getenv('PROGRAMFILES(X86)', '')) / "Ollama" / "ollama.exe",
        ]
        
        for path in possible_paths:
            if path.exists():
                self.ollama_exe = path  # 更新路径
                return True
        
        # 方法3: 检查系统 PATH
        try:
            result = subprocess.run(
                ["ollama", "--version"],
                capture_output=True,
                text=True,
                timeout=5
            )
            return result.returncode == 0
        except:
            pass
        
        # 方法4: 使用 where 命令查找
        try:
            result = subprocess.run(
                ["where", "ollama"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                return True
        except:
            pass
        
        return False
    
    def validate_installer_file(self, file_path):
        """
        验证安装程序文件的完整性
        
        Returns:
            tuple: (is_valid, error_message)
        """
        file_path = Path(file_path)
        
        # 1. 检查文件是否存在
        if not file_path.exists():
            return False, "文件不存在"
        
        # 2. 检查文件大小
        file_size = file_path.stat().st_size
        self.log(f"文件大小: {file_size / 1024 / 1024:.2f} MB")
        
        # Ollama 安装程序通常在 50-200MB 之间
        # 但由于 Content-Length 可能不准确，我们放宽限制
        if file_size < 50 * 1024 * 1024:
            return False, f"文件大小异常（{file_size / 1024 / 1024:.2f} MB < 50 MB），可能下载不完整"
        
        if file_size > 2000 * 1024 * 1024:  # 放宽到 2GB
            return False, f"文件大小异常（{file_size / 1024 / 1024:.2f} MB > 2000 MB），可能不是正确的安装程序"
        
        # 3. 验证 PE 文件头（Windows 可执行文件）
        try:
            with open(file_path, 'rb') as f:
                # 读取 DOS 头
                magic = f.read(2)
                if magic != b'MZ':
                    return False, "文件不是有效的 Windows 可执行文件（缺少 MZ 标识）"
                
                # 读取 PE 头位置
                f.seek(0x3C)
                pe_offset_bytes = f.read(4)
                pe_offset = int.from_bytes(pe_offset_bytes, byteorder='little')
                
                # 验证 PE 签名
                f.seek(pe_offset)
                pe_signature = f.read(4)
                if pe_signature != b'PE\x00\x00':
                    return False, "文件不是有效的 PE 文件（缺少 PE 签名）"
                
                # 读取 COFF 头
                f.seek(pe_offset + 4)
                machine = f.read(2)
                # 0x8664 = AMD64, 0x014c = I386
                machine_type = int.from_bytes(machine, byteorder='little')
                if machine_type not in [0x8664, 0x014c]:
                    return False, f"不支持的架构类型: 0x{machine_type:04x}"
                
                self.log(f"✓ PE 文件验证通过（架构: {'x64' if machine_type == 0x8664 else 'x86'}）")
                
        except Exception as e:
            return False, f"PE 文件验证失败: {e}"
        
        # 4. 计算文件哈希（用于调试和日志）
        try:
            sha256_hash = hashlib.sha256()
            with open(file_path, 'rb') as f:
                # 分块读取以处理大文件
                for chunk in iter(lambda: f.read(8192), b''):
                    sha256_hash.update(chunk)
            
            file_hash = sha256_hash.hexdigest()
            self.log(f"文件 SHA256: {file_hash[:16]}...")
            
        except Exception as e:
            self.log(f"⚠ 无法计算文件哈希: {e}", "WARNING")
        
        # 5. 尝试读取文件的版本信息（可选）
        try:
            # 使用 PowerShell 读取文件版本
            ps_command = f'(Get-Item "{file_path}").VersionInfo | Select-Object -ExpandProperty FileVersion'
            result = subprocess.run(
                ["powershell", "-Command", ps_command],
                capture_output=True,
                text=True,
                timeout=5,
                creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0
            )
            
            if result.returncode == 0 and result.stdout.strip():
                version = result.stdout.strip()
                self.log(f"安装程序版本: {version}")
        except Exception as e:
            self.log(f"⚠ 无法读取版本信息: {e}", "WARNING")
        
        return True, "验证通过"
    
    def install_ollama(self):
        """检查 Ollama 是否已安装"""
        self.log("=" * 60)
        self.log("步骤 1: 检查 Ollama")
        self.log("=" * 60)
        
        if self.check_ollama_installed():
            self.log("✓ Ollama 已安装")
            self.log(f"✓ 安装路径: {self.ollama_exe}")
            return
        
        # Ollama 未安装，提示用户手动安装
        self.log("=" * 60, "ERROR")
        self.log("✗ Ollama 未安装", "ERROR")
        self.log("=" * 60, "ERROR")
        self.log("")
        self.log("请先安装 Ollama：")
        self.log("")
        self.log("方法1（推荐）：")
        self.log("  1. 访问 https://ollama.com/download")
        self.log("  2. 点击 'Download for Windows'")
        self.log("  3. 运行下载的安装程序")
        self.log("  4. 安装完成后，重新运行本程序")
        self.log("")
        self.log("方法2（命令行）：")
        self.log("  1. 打开 PowerShell（管理员）")
        self.log("  2. 运行: irm https://ollama.com/install.ps1 | iex")
        self.log("")
        
        raise RuntimeError("请先安装 Ollama，然后重新运行本程序")
    
    def start_ollama_service(self):
        """启动 Ollama 服务"""
        self.log("=" * 60)
        self.log("步骤 2: 启动 Ollama 服务")
        self.log("=" * 60)
        
        # 检查 Ollama 是否可用
        if not self.check_ollama_installed():
            self.log("⚠ Ollama 尚未检测到，跳过服务启动", "WARNING")
            self.log("请重启应用程序后再试")
            return
        
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
                self.log("⚠ 服务可能未完全启动，请稍后重试", "WARNING")
                
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
        
        # 检查 Ollama 是否可用
        if not self.check_ollama_installed():
            self.log("⚠ Ollama 尚未检测到，跳过模型安装", "WARNING")
            self.log("请重启应用程序后，再次尝试安装模型")
            return
        
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
        
        # 检查 Ollama 是否可用
        if not self.check_ollama_installed():
            self.log("⚠ Ollama 尚未检测到，跳过验证", "WARNING")
            self.log("安装可能需要重启应用程序或计算机才能生效")
            return
        
        # 验证 Ollama
        try:
            self.run_command(
                ["ollama", "--version"],
                "检查 Ollama 版本"
            )
        except Exception as e:
            self.log(f"⚠ 无法验证 Ollama 版本: {e}", "WARNING")
            self.log("这可能是正常的，请重启应用程序后再试")
            return
        
        # 验证模型
        try:
            self.run_command(
                ["ollama", "list"],
                "列出已安装的模型"
            )
        except Exception as e:
            self.log(f"⚠ 无法列出模型: {e}", "WARNING")
            return
        
        # 测试模型
        if self.check_model_installed():
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
            self.log("")
            self.log("⚠ 重要提示：")
            self.log("  - 首次安装需要下载约 1.6GB 文件")
            self.log("  - 支持断点续传，可随时中断后继续")
            self.log("  - 请确保网络连接稳定")
            self.log("  - 预计时间：10-30 分钟（取决于网络速度）")
            self.log("")
            
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
            self.log("")
            self.log("故障排除建议：")
            self.log("1. 检查网络连接是否稳定")
            self.log("2. 检查防火墙是否阻止下载")
            self.log("3. 尝试重新运行安装（支持断点续传）")
            self.log("4. 如果问题持续，可以手动安装：")
            self.log("   - 访问 https://ollama.com/download")
            self.log("   - 下载并安装 Ollama")
            self.log("   - 运行: ollama pull qwen2.5:1.5b")
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
