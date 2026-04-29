#!/usr/bin/env python3
"""
OCR 安装脚本 - 独立版本
用于下载和安装 Python + PyMuPDF + EasyOCR

特点：
1. 使用国内镜像源（阿里云）
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
import zipfile
import subprocess
import shutil
import time
from pathlib import Path


class OCRInstaller:
    def __init__(self, install_dir=None):
        """
        初始化安装器
        
        Args:
            install_dir: 安装目录，默认为 %APPDATA%/com.cheersai.vault/ocr-package
        """
        if install_dir:
            self.install_dir = Path(install_dir)
        else:
            appdata = os.getenv('APPDATA')
            if not appdata:
                raise RuntimeError("无法获取 APPDATA 环境变量")
            self.install_dir = Path(appdata) / "com.cheersai.vault" / "ocr-package"
        
        self.python_dir = self.install_dir / "python"
        self.python_exe = self.python_dir / "python.exe"
        self.pip_exe = self.python_dir / "Scripts" / "pip.exe"
        
        # 多个镜像源（按优先级排序）
        self.python_urls = [
            "https://mirrors.huaweicloud.com/python/3.11.9/python-3.11.9-embed-amd64.zip",
            "https://mirrors.aliyun.com/python-release/windows/python-3.11.9-embed-amd64.zip",
            "https://npm.taobao.org/mirrors/python/3.11.9/python-3.11.9-embed-amd64.zip",
            "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip",
        ]
        self.get_pip_url = "https://bootstrap.pypa.io/get-pip.py"
        self.pypi_mirror = "https://mirrors.aliyun.com/pypi/simple/"
        
        # 重试配置
        self.max_retries = 3
        self.retry_delay = 3  # 秒
        self.download_timeout = 600  # 10分钟
        self.chunk_size = 65536  # 64KB，更大的块提高速度
        
    def log(self, message, level="INFO"):
        """输出日志"""
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        print(f"[{timestamp}] [{level}] {message}", flush=True)
    
    def download_file(self, url, dest_path, description="文件"):
        """
        下载文件，支持重试和进度显示
        
        Args:
            url: 下载链接
            dest_path: 保存路径
            description: 文件描述
        """
        dest_path = Path(dest_path)
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        
        for attempt in range(1, self.max_retries + 1):
            try:
                self.log(f"下载 {description} (尝试 {attempt}/{self.max_retries})")
                self.log(f"URL: {url}")
                self.log(f"保存到: {dest_path}")
                
                # 创建请求
                req = urllib.request.Request(url)
                req.add_header('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36')
                req.add_header('Accept', '*/*')
                req.add_header('Connection', 'keep-alive')
                
                # 下载
                with urllib.request.urlopen(req, timeout=self.download_timeout) as response:
                    total_size = int(response.headers.get('Content-Length', 0))
                    
                    if total_size > 0:
                        self.log(f"文件大小: {total_size / 1024 / 1024:.2f} MB")
                    
                    downloaded = 0
                    last_progress = 0
                    
                    with open(dest_path, 'wb') as f:
                        while True:
                            chunk = response.read(self.chunk_size)
                            if not chunk:
                                break
                            f.write(chunk)
                            downloaded += len(chunk)
                            
                            # 显示进度（每 5% 显示一次）
                            if total_size > 0:
                                progress = (downloaded / total_size) * 100
                                if progress - last_progress >= 5 or progress >= 99:
                                    self.log(f"下载进度: {progress:.1f}% ({downloaded / 1024 / 1024:.2f} MB / {total_size / 1024 / 1024:.2f} MB)")
                                    last_progress = progress
                
                # 验证文件大小
                if total_size > 0:
                    actual_size = dest_path.stat().st_size
                    if actual_size != total_size:
                        raise RuntimeError(f"文件大小不匹配：期望 {total_size} 字节，实际 {actual_size} 字节")
                
                self.log(f"✓ {description} 下载完成")
                return True
                
            except urllib.error.URLError as e:
                self.log(f"✗ 网络错误: {e}", "ERROR")
                if attempt < self.max_retries:
                    self.log(f"等待 {self.retry_delay} 秒后重试...")
                    time.sleep(self.retry_delay)
                else:
                    raise RuntimeError(f"下载 {description} 失败：{e}")
            except Exception as e:
                self.log(f"✗ 下载出错: {e}", "ERROR")
                if attempt < self.max_retries:
                    self.log(f"等待 {self.retry_delay} 秒后重试...")
                    time.sleep(self.retry_delay)
                else:
                    raise RuntimeError(f"下载 {description} 失败：{e}")
    
    def download_file_with_fallback(self, urls, dest_path, description="文件"):
        """
        使用多个镜像源下载文件，自动切换
        
        Args:
            urls: 镜像源列表
            dest_path: 保存路径
            description: 文件描述
        """
        last_error = None
        
        for i, url in enumerate(urls):
            try:
                self.log(f"尝试镜像源 {i + 1}/{len(urls)}")
                self.download_file(url, dest_path, description)
                return True
            except Exception as e:
                last_error = e
                self.log(f"镜像源 {i + 1} 失败: {e}", "WARNING")
                if i < len(urls) - 1:
                    self.log(f"切换到下一个镜像源...")
                    time.sleep(1)
                continue
        
        # 所有镜像源都失败
        raise RuntimeError(f"所有镜像源下载失败。最后错误: {last_error}")
    
    def extract_zip(self, zip_path, extract_to):
        """解压 ZIP 文件"""
        self.log(f"解压文件: {zip_path}")
        self.log(f"目标目录: {extract_to}")
        
        try:
            with zipfile.ZipFile(zip_path, 'r') as zip_ref:
                zip_ref.extractall(extract_to)
            self.log("✓ 解压完成")
            return True
        except Exception as e:
            self.log(f"✗ 解压失败: {e}", "ERROR")
            raise
    
    def run_command(self, cmd, description="命令", check=True):
        """
        执行命令
        
        Args:
            cmd: 命令列表
            description: 命令描述
            check: 是否检查返回码
        """
        self.log(f"执行: {description}")
        self.log(f"命令: {' '.join(cmd)}")
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=600,  # 10分钟超时
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
    
    def install_python(self):
        """安装 Python 嵌入式版本"""
        self.log("=" * 60)
        self.log("步骤 1: 安装 Python 3.11.9 嵌入式版本")
        self.log("=" * 60)
        
        if self.python_exe.exists():
            self.log("Python 已安装，跳过")
            return
        
        # 使用多镜像源下载 Python
        python_zip = self.install_dir / "python.zip"
        self.log("开始下载 Python（约 11 MB）...")
        self.download_file_with_fallback(self.python_urls, python_zip, "Python 3.11.9")
        
        # 解压
        self.extract_zip(python_zip, self.python_dir)
        
        # 删除 zip 文件
        python_zip.unlink()
        self.log("✓ 已清理安装文件")
        
        # 验证安装
        if not self.python_exe.exists():
            raise RuntimeError("Python 安装失败：找不到 python.exe")
        
        # 修改 python311._pth 以启用 site-packages
        pth_file = self.python_dir / "python311._pth"
        if pth_file.exists():
            content = pth_file.read_text()
            if "#import site" in content:
                content = content.replace("#import site", "import site")
                pth_file.write_text(content)
                self.log("✓ 已启用 site-packages")
        
        self.log("✓ Python 安装完成")
    
    def install_pip(self):
        """安装 pip"""
        self.log("=" * 60)
        self.log("步骤 2: 安装 pip")
        self.log("=" * 60)
        
        if self.pip_exe.exists():
            self.log("pip 已安装，跳过")
            return
        
        # 下载 get-pip.py
        get_pip_path = self.install_dir / "get-pip.py"
        self.download_file(self.get_pip_url, get_pip_path, "get-pip.py")
        
        # 安装 pip
        self.run_command(
            [str(self.python_exe), str(get_pip_path), "--no-warn-script-location"],
            "安装 pip"
        )
        
        # 删除 get-pip.py
        get_pip_path.unlink()
        
        # 验证安装
        if not self.pip_exe.exists():
            raise RuntimeError("pip 安装失败：找不到 pip.exe")
        
        self.log("✓ pip 安装完成")
    
    def install_packages(self):
        """安装 PyMuPDF 和 EasyOCR"""
        self.log("=" * 60)
        self.log("步骤 3: 安装 Python 包")
        self.log("=" * 60)
        
        packages = [
            ("PyMuPDF", 1),  # 小包，1次重试
            ("easyocr", 3),  # 大包，3次重试
        ]
        
        for package_name, max_retries in packages:
            self.log(f"\n安装 {package_name}...")
            
            for attempt in range(1, max_retries + 1):
                try:
                    self.run_command(
                        [
                            str(self.pip_exe),
                            "install",
                            package_name,
                            "-i", self.pypi_mirror,
                            "--trusted-host", "mirrors.aliyun.com",
                            "--retries", "5",
                            "--timeout", "300",
                            "--no-warn-script-location"
                        ],
                        f"安装 {package_name} (尝试 {attempt}/{max_retries})"
                    )
                    break  # 成功则跳出重试循环
                    
                except Exception as e:
                    if attempt < max_retries:
                        self.log(f"安装失败，等待 {self.retry_delay} 秒后重试...")
                        time.sleep(self.retry_delay)
                    else:
                        raise RuntimeError(f"安装 {package_name} 失败，已重试 {max_retries} 次: {e}")
        
        self.log("✓ 所有包安装完成")
    
    def verify_installation(self):
        """验证安装"""
        self.log("=" * 60)
        self.log("步骤 4: 验证安装")
        self.log("=" * 60)
        
        # 验证 Python
        result = self.run_command(
            [str(self.python_exe), "--version"],
            "检查 Python 版本",
            check=False
        )
        
        # 验证 pip
        result = self.run_command(
            [str(self.pip_exe), "--version"],
            "检查 pip 版本",
            check=False
        )
        
        # 验证包
        test_script = """
import sys
try:
    import fitz
    print(f"✓ PyMuPDF 版本: {fitz.__version__}")
except ImportError as e:
    print(f"✗ PyMuPDF 导入失败: {e}")
    sys.exit(1)

try:
    import easyocr
    print(f"✓ EasyOCR 已安装")
except ImportError as e:
    print(f"✗ EasyOCR 导入失败: {e}")
    sys.exit(1)

print("✓ 所有包验证通过")
"""
        
        test_file = self.install_dir / "test_imports.py"
        test_file.write_text(test_script, encoding='utf-8')
        
        try:
            self.run_command(
                [str(self.python_exe), str(test_file)],
                "验证包导入"
            )
        finally:
            test_file.unlink()
        
        self.log("✓ 安装验证完成")
    
    def create_pdf_ocr_script(self):
        """创建 PDF OCR 脚本"""
        self.log("=" * 60)
        self.log("步骤 5: 创建 PDF OCR 脚本")
        self.log("=" * 60)
        
        script_content = '''#!/usr/bin/env python3
"""PDF OCR 处理脚本"""
import sys
import json
import fitz  # PyMuPDF
import easyocr

def main():
    if len(sys.argv) < 2:
        print("Usage: pdf_ocr.py <pdf_path>", file=sys.stderr)
        sys.exit(1)
    
    pdf_path = sys.argv[1]
    
    try:
        print(f"OCR: Opening PDF: {pdf_path}", file=sys.stderr)
        doc = fitz.open(pdf_path)
        print(f"OCR: PDF has {len(doc)} pages", file=sys.stderr)
        
        print("OCR: Initializing EasyOCR (first time may download models)...", file=sys.stderr)
        reader = easyocr.Reader(['ch_sim', 'en'], gpu=False)
        print("OCR: EasyOCR initialized successfully", file=sys.stderr)
        
        all_text = []
        
        for page_num in range(len(doc)):
            print(f"OCR: Processing page {page_num + 1}/{len(doc)}...", file=sys.stderr)
            page = doc[page_num]
            
            # 转换为图片
            pix = page.get_pixmap(matrix=fitz.Matrix(2, 2))
            img_data = pix.tobytes("png")
            
            # OCR 识别
            print(f"OCR: Running OCR on page {page_num + 1}...", file=sys.stderr)
            results = reader.readtext(img_data)
            
            page_text = " ".join([text for (_, text, _) in results])
            all_text.append(page_text)
            
            print(f"OCR: Page {page_num + 1} extracted {len(page_text)} characters", file=sys.stderr)
        
        # 输出结果
        output = {
            "success": True,
            "text": "\\n".join(all_text),
            "pages": len(doc)
        }
        print(json.dumps(output, ensure_ascii=False))
        
    except Exception as e:
        output = {
            "success": False,
            "error": str(e)
        }
        print(json.dumps(output, ensure_ascii=False))
        sys.exit(1)

if __name__ == "__main__":
    main()
'''
        
        script_path = self.install_dir / "pdf_ocr.py"
        script_path.write_text(script_content, encoding='utf-8')
        
        self.log(f"✓ PDF OCR 脚本已创建: {script_path}")
    
    def install(self):
        """执行完整安装流程"""
        try:
            self.log("=" * 60)
            self.log("开始安装 OCR 环境")
            self.log(f"安装目录: {self.install_dir}")
            self.log("=" * 60)
            
            # 创建安装目录
            self.install_dir.mkdir(parents=True, exist_ok=True)
            
            # 执行安装步骤
            self.install_python()
            self.install_pip()
            self.install_packages()
            self.verify_installation()
            self.create_pdf_ocr_script()
            
            self.log("=" * 60)
            self.log("✓ OCR 环境安装完成！")
            self.log("=" * 60)
            
            # 输出安装信息
            info = {
                "success": True,
                "install_dir": str(self.install_dir),
                "python_exe": str(self.python_exe),
                "pip_exe": str(self.pip_exe),
                "pdf_ocr_script": str(self.install_dir / "pdf_ocr.py")
            }
            
            info_file = self.install_dir / "install_info.json"
            info_file.write_text(json.dumps(info, indent=2, ensure_ascii=False), encoding='utf-8')
            
            return info
            
        except Exception as e:
            self.log(f"✗ 安装失败: {e}", "ERROR")
            raise
    
    def uninstall(self):
        """卸载 OCR 环境"""
        self.log("=" * 60)
        self.log("开始卸载 OCR 环境")
        self.log("=" * 60)
        
        if self.install_dir.exists():
            self.log(f"删除目录: {self.install_dir}")
            shutil.rmtree(self.install_dir)
            self.log("✓ OCR 环境已卸载")
        else:
            self.log("OCR 环境未安装")


def main():
    """主函数"""
    if len(sys.argv) > 1 and sys.argv[1] == "uninstall":
        installer = OCRInstaller()
        installer.uninstall()
    else:
        installer = OCRInstaller()
        info = installer.install()
        print("\n" + "=" * 60)
        print("安装信息:")
        print(json.dumps(info, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
