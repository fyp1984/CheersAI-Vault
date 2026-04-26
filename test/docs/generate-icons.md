# 图标生成指南

## 使用您的 logo.jpg 生成应用图标

### 方法一：使用在线工具
1. 访问 https://www.favicon-generator.org/ 或 https://realfavicongenerator.net/
2. 上传您的 `public/logo.jpg` 文件
3. 生成各种尺寸的图标
4. 下载生成的图标包

### 方法二：使用 ImageMagick (推荐)
如果您安装了 ImageMagick，可以使用以下命令：

```bash
# 进入图标目录
cd src-tauri/icons

# 从 logo.jpg 生成各种尺寸
magick ../../public/logo.jpg -resize 32x32 32x32.png
magick ../../public/logo.jpg -resize 128x128 128x128.png
magick ../../public/logo.jpg -resize 256x256 128x128@2x.png
magick ../../public/logo.jpg -resize 512x512 icon.png

# 生成 ICO 文件 (Windows)
magick ../../public/logo.jpg -resize 256x256 icon.ico

# 生成 ICNS 文件 (macOS) - 需要额外步骤
mkdir icon.iconset
magick ../../public/logo.jpg -resize 16x16 icon.iconset/icon_16x16.png
magick ../../public/logo.jpg -resize 32x32 icon.iconset/icon_16x16@2x.png
magick ../../public/logo.jpg -resize 32x32 icon.iconset/icon_32x32.png
magick ../../public/logo.jpg -resize 64x64 icon.iconset/icon_32x32@2x.png
magick ../../public/logo.jpg -resize 128x128 icon.iconset/icon_128x128.png
magick ../../public/logo.jpg -resize 256x256 icon.iconset/icon_128x128@2x.png
magick ../../public/logo.jpg -resize 256x256 icon.iconset/icon_256x256.png
magick ../../public/logo.jpg -resize 512x512 icon.iconset/icon_256x256@2x.png
magick ../../public/logo.jpg -resize 512x512 icon.iconset/icon_512x512.png
magick ../../public/logo.jpg -resize 1024x1024 icon.iconset/icon_512x512@2x.png
iconutil -c icns icon.iconset
rm -rf icon.iconset

# Windows Store 图标
magick ../../public/logo.jpg -resize 30x30 Square30x30Logo.png
magick ../../public/logo.jpg -resize 44x44 Square44x44Logo.png
magick ../../public/logo.jpg -resize 71x71 Square71x71Logo.png
magick ../../public/logo.jpg -resize 89x89 Square89x89Logo.png
magick ../../public/logo.jpg -resize 107x107 Square107x107Logo.png
magick ../../public/logo.jpg -resize 142x142 Square142x142Logo.png
magick ../../public/logo.jpg -resize 150x150 Square150x150Logo.png
magick ../../public/logo.jpg -resize 284x284 Square284x284Logo.png
magick ../../public/logo.jpg -resize 310x310 Square310x310Logo.png
magick ../../public/logo.jpg -resize 50x50 StoreLogo.png
```

### 方法三：手动处理
1. 使用图像编辑软件（如 Photoshop、GIMP）
2. 打开 `public/logo.jpg`
3. 调整尺寸并导出为以下格式：
   - 32x32.png
   - 128x128.png
   - 128x128@2x.png (256x256)
   - icon.png (512x512)
   - icon.ico (256x256)
   - icon.icns (使用在线转换工具)

### 需要生成的文件列表
- `32x32.png` - 32x32 像素
- `128x128.png` - 128x128 像素  
- `128x128@2x.png` - 256x256 像素
- `icon.png` - 512x512 像素
- `icon.ico` - Windows 图标
- `icon.icns` - macOS 图标
- Windows Store 图标（各种尺寸）

## 完成后
生成图标后，重新编译应用：
```bash
pnpm tauri build
```

图标将自动应用到：
- 应用程序图标
- 任务栏图标  
- 桌面快捷方式
- 安装包图标