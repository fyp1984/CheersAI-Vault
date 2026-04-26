/**
 * 跨平台路径处理工具
 */

// 检测当前平台
export const getPlatform = (): 'windows' | 'macos' | 'linux' => {
  // 使用 userAgent 替代已弃用的 platform
  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes('win')) return 'windows';
  if (userAgent.includes('mac')) return 'macos';
  return 'linux';
};

// 路径分隔符
export const getPathSeparator = (): string => {
  return getPlatform() === 'windows' ? '\\' : '/';
};

// 标准化路径分隔符
export const normalizePath = (path: string): string => {
  if (!path) return '';
  
  const platform = getPlatform();
  if (platform === 'windows') {
    // Windows: 统一使用反斜杠
    return path.replace(/\//g, '\\');
  } else {
    // Unix-like: 统一使用正斜杠
    return path.replace(/\\/g, '/');
  }
};

// 连接路径
export const joinPath = (...parts: string[]): string => {
  const separator = getPathSeparator();
  return parts
    .filter(part => part && part.length > 0)
    .map(part => part.replace(/[/\\]+$/, '')) // 移除末尾的分隔符
    .join(separator);
};

// 获取文件名（不含路径）
export const getFileName = (path: string): string => {
  if (!path) return '';
  const normalized = normalizePath(path);
  const separator = getPathSeparator();
  const parts = normalized.split(separator);
  return parts[parts.length - 1] || '';
};

// 获取目录路径（不含文件名）
export const getDirectoryPath = (path: string): string => {
  if (!path) return '';
  const normalized = normalizePath(path);
  const separator = getPathSeparator();
  const parts = normalized.split(separator);
  parts.pop(); // 移除文件名
  return parts.join(separator);
};

// 检查路径是否为绝对路径
export const isAbsolutePath = (path: string): boolean => {
  if (!path) return false;
  
  const platform = getPlatform();
  if (platform === 'windows') {
    // Windows: C:\ 或 \\server\share
    return /^[a-zA-Z]:[/\\]/.test(path) || /^[/\\]{2}/.test(path);
  } else {
    // Unix-like: 以 / 开头
    return path.startsWith('/');
  }
};

// 获取应用数据目录
export const getAppDataPath = (): string => {
  const platform = getPlatform();
  switch (platform) {
    case 'windows':
      return '%APPDATA%\\CheersAI Vault';
    case 'macos':
      return '~/Library/Application Support/CheersAI Vault';
    case 'linux':
      return '~/.config/CheersAI Vault';
    default:
      return './CheersAI Vault';
  }
};

// 获取默认文档目录
export const getDefaultDocumentsPath = (): string => {
  const platform = getPlatform();
  switch (platform) {
    case 'windows':
      return '%USERPROFILE%\\Documents\\CheersAI Vault';
    case 'macos':
      return '~/Documents/CheersAI Vault';
    case 'linux':
      return '~/Documents/CheersAI Vault';
    default:
      return './CheersAI Vault';
  }
};

// 获取临时目录
export const getTempPath = (): string => {
  const platform = getPlatform();
  switch (platform) {
    case 'windows':
      return '%TEMP%\\CheersAI Vault';
    case 'macos':
      return '/tmp/CheersAI Vault';
    case 'linux':
      return '/tmp/CheersAI Vault';
    default:
      return './temp';
  }
};

// 获取日志目录
export const getLogPath = (): string => {
  const platform = getPlatform();
  switch (platform) {
    case 'windows':
      return joinPath(getAppDataPath(), 'logs');
    case 'macos':
      return '~/Library/Logs/CheersAI Vault';
    case 'linux':
      return '~/.local/share/CheersAI Vault/logs';
    default:
      return './logs';
  }
};

// 获取配置文件路径
export const getConfigPath = (filename: string = 'config.json'): string => {
  return joinPath(getAppDataPath(), filename);
};

// 获取缓存目录
export const getCachePath = (): string => {
  const platform = getPlatform();
  switch (platform) {
    case 'windows':
      return '%LOCALAPPDATA%\\CheersAI Vault\\Cache';
    case 'macos':
      return '~/Library/Caches/CheersAI Vault';
    case 'linux':
      return '~/.cache/CheersAI Vault';
    default:
      return './cache';
  }
};

// 显示友好的路径（缩短长路径）
export const getDisplayPath = (path: string, maxLength: number = 50): string => {
  if (!path || path.length <= maxLength) return path;
  
  const separator = getPathSeparator();
  const parts = normalizePath(path).split(separator);
  
  if (parts.length <= 2) return path;
  
  // 保留开头和结尾部分
  const start = parts[0];
  const end = parts[parts.length - 1];
  const middle = '...';
  
  let result = `${start}${separator}${middle}${separator}${end}`;
  
  // 如果还是太长，进一步缩短
  if (result.length > maxLength && end.length > 20) {
    const shortEnd = end.substring(0, 15) + '...';
    result = `${start}${separator}${middle}${separator}${shortEnd}`;
  }
  
  return result;
};

// 验证路径格式
export const validatePath = (path: string): { valid: boolean; error?: string } => {
  if (!path) {
    return { valid: false, error: '路径不能为空' };
  }
  
  const platform = getPlatform();
  
  // 检查非法字符
  const invalidChars = platform === 'windows' 
    ? /[<>"|?*]/ // Windows: 不包含冒号，因为C:\是合法的
    : /[\0]/; // Unix-like: 只检查null字符
    
  if (invalidChars.test(path)) {
    return { valid: false, error: '路径包含非法字符' };
  }
  
  // Windows 特殊检查
  if (platform === 'windows') {
    // 检查保留名称 - 只检查完整的路径组件
    const reservedNames = ['CON', 'PRN', 'AUX', 'NUL', 'COM1', 'COM2', 'COM3', 'COM4', 'COM5', 'COM6', 'COM7', 'COM8', 'COM9', 'LPT1', 'LPT2', 'LPT3', 'LPT4', 'LPT5', 'LPT6', 'LPT7', 'LPT8', 'LPT9'];
    const pathParts = path.split(/[/\\]/);
    for (const part of pathParts) {
      if (!part) continue; // 跳过空部分
      // 获取不含扩展名的文件名
      const baseName = part.includes('.') ? part.substring(0, part.lastIndexOf('.')) : part;
      // 只有当完全匹配保留名称时才报错
      if (reservedNames.includes(baseName.toUpperCase())) {
        return { valid: false, error: `包含Windows保留名称: ${part}` };
      }
    }
    
    // 检查驱动器格式（如果是绝对路径）
    if (path.length >= 2 && path[1] === ':') {
      const drive = path[0].toUpperCase();
      if (!/[A-Z]/.test(drive)) {
        return { valid: false, error: '无效的驱动器字母' };
      }
    }
  }
  
  return { valid: true };
};

// 解析环境变量路径
export const resolveEnvPath = (path: string): string => {
  if (!path) return '';
  
  const platform = getPlatform();
  let resolved = path;
  
  if (platform === 'windows') {
    // Windows 环境变量
    resolved = resolved
      .replace(/%USERPROFILE%/g, '~')
      .replace(/%APPDATA%/g, '~/AppData/Roaming')
      .replace(/%LOCALAPPDATA%/g, '~/AppData/Local')
      .replace(/%TEMP%/g, '~/AppData/Local/Temp')
      .replace(/%USERNAME%/g, 'User');
  }
  
  // Unix-like 环境变量
  resolved = resolved.replace(/~/g, '/Users/User'); // 简化显示
  
  return normalizePath(resolved);
};

// 创建安全的文件名
export const sanitizeFileName = (filename: string): string => {
  if (!filename) return '';
  
  const platform = getPlatform();
  let sanitized = filename;
  
  if (platform === 'windows') {
    // Windows 非法字符
    sanitized = sanitized.replace(/[<>:"|?*\\\/]/g, '_');
    
    // Windows 保留名称
    const reservedNames = ['CON', 'PRN', 'AUX', 'NUL', 'COM1', 'COM2', 'COM3', 'COM4', 'COM5', 'COM6', 'COM7', 'COM8', 'COM9', 'LPT1', 'LPT2', 'LPT3', 'LPT4', 'LPT5', 'LPT6', 'LPT7', 'LPT8', 'LPT9'];
    const baseName = sanitized.split('.')[0].toUpperCase();
    if (reservedNames.includes(baseName)) {
      sanitized = `_${sanitized}`;
    }
  } else {
    // Unix-like 非法字符
    sanitized = sanitized.replace(/[\/\0]/g, '_');
  }
  
  // 移除开头和结尾的空格和点
  sanitized = sanitized.replace(/^[\s.]+|[\s.]+$/g, '');
  
  // 确保不为空
  if (!sanitized) {
    sanitized = 'untitled';
  }
  
  return sanitized;
};

// 获取文件扩展名
export const getFileExtension = (filename: string): string => {
  if (!filename) return '';
  const lastDot = filename.lastIndexOf('.');
  return lastDot > 0 ? filename.substring(lastDot + 1).toLowerCase() : '';
};

// 构建输出文件路径
export const buildOutputPath = (inputPath: string, outputDir: string, prefix: string = 'masked_'): string => {
  const fileName = getFileName(inputPath);
  const sanitizedName = sanitizeFileName(`${prefix}${fileName}`);
  return joinPath(outputDir, sanitizedName);
};