# 敏感词实体快速操作功能

## 功能概述

在文件脱敏预览的"手动查找替换"对话框中，为每个识别到的敏感词实体添加了快速操作按钮：
- **左上角绿色 ✓ 按钮**：快速添加到替换列表
- **右上角红色 ✗ 按钮**：删除识别错误的实体

## 功能特性

### 1. 快速添加按钮（左上角 ✓）
- 绿色圆形按钮，位于 Badge 左上角（`-top-1 -left-1`）
- 点击后直接添加到替换列表，使用默认的替换规则
- 适用于确认正确的敏感信息，快速添加脱敏规则

### 2. 快速删除按钮（右上角 ✗）
- 红色圆形按钮，位于 Badge 右上角（`-top-1 -right-1`）
- 点击后从识别列表中移除该实体
- 适用于移除识别错误的内容

### 3. 交互行为
- **点击 Badge 主体**：添加该实体到替换列表（原有功能）
- **点击左上角 ✓**：快速添加到替换列表（新功能）
- **点击右上角 ✗**：从识别列表中移除
- 所有按钮默认隐藏，鼠标悬停时显示（`group-hover` 效果）
- 使用 `stopPropagation()` 阻止事件冒泡

### 3. 状态管理
- 使用本地状态 `localEntities` 管理当前显示的实体列表
- 使用 `removedEntities` Set 记录已删除的实体文本
- 删除操作会同时更新本地状态和通知父组件

## 实现细节

### 组件修改

#### FindReplaceDialog.tsx
```typescript
// 新增 props
interface FindReplaceDialogProps {
  // ... 其他 props
  onRemoveEntity?: (text: string) => void;  // 删除实体回调
}

// 本地状态管理
const [localEntities, setLocalEntities] = useState(detectedEntities);

// 删除处理函数
const handleRemoveEntity = (text: string, e: React.MouseEvent) => {
  e.stopPropagation(); // 阻止冒泡
  setLocalEntities(prev => prev.filter(entity => entity.text !== text));
  if (onRemoveEntity) {
    onRemoveEntity(text);
  }
};
```

#### MaskingPreviewDialog.tsx
```typescript
// 记录已删除的实体
const [removedEntities, setRemovedEntities] = useState<Set<string>>(new Set());

// 过滤已删除的实体
const allEntities = (preview.detected_entities?.flatMap(row => row.entities) || [])
  .filter(entity => !removedEntities.has(entity.text));

// 删除处理
const handleRemoveEntity = (text: string) => {
  setRemovedEntities(prev => new Set([...prev, text]));
};
```

### UI 样式

```tsx
<Badge
  className="cursor-pointer hover:bg-blue-100 border-blue-200 transition-colors text-xs relative group pr-6 pl-6"
  onClick={() => handleQuickSelect(entity.text, entity.entity_type)}
>
  {/* 左上角：快速添加按钮 */}
  <button
    onClick={(e) => handleQuickAdd(entity.text, entity.entity_type, e)}
    className="absolute -top-1 -left-1 w-4 h-4 bg-green-500 hover:bg-green-600 text-white rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
    title="添加到替换列表"
  >
    <Check className="w-3 h-3" />
  </button>
  
  <span className="text-orange-700 font-medium mr-1">{entity.entity_type}:</span>
  <span className="text-gray-700">{entity.text}</span>
  
  {/* 右上角：删除按钮 */}
  <button
    onClick={(e) => handleRemoveEntity(entity.text, e)}
    className="absolute -top-1 -right-1 w-4 h-4 bg-red-500 hover:bg-red-600 text-white rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
    title="删除此识别结果"
  >
    <X className="w-3 h-3" />
  </button>
</Badge>
```

## 使用场景

### 场景 1: 快速添加正确的敏感信息
当识别结果正确时，可以快速添加到替换列表：

1. 打开文件脱敏预览
2. 点击"手动查找替换"按钮
3. 在识别到的敏感词列表中，鼠标悬停在正确识别的实体上
4. 点击左上角的 ✓ 按钮
5. 该实体自动添加到替换列表，使用默认的替换规则

### 场景 2: 移除误识别的实体
当 NER 模型错误地将某些文本识别为敏感信息时：

1. 打开文件脱敏预览
2. 点击"手动查找替换"按钮
3. 在识别到的敏感词列表中，鼠标悬停在误识别的实体上
4. 点击右上角的 ✗ 按钮
5. 该实体从列表中移除

### 场景 3: 批量处理混合结果
对于包含正确和错误识别的文件：

1. 先点击 ✓ 快速添加所有正确的敏感信息
2. 再点击 ✗ 删除所有误识别的内容
3. 最后调整替换规则（如需要）
4. 执行全部替换

## 技术要点

### 1. 事件冒泡处理
```typescript
const handleRemoveEntity = (text: string, e: React.MouseEvent) => {
  e.stopPropagation(); // 关键：阻止触发 Badge 的 onClick
  // ... 删除逻辑
};
```

### 2. 状态同步
使用 `useEffect` 确保当 `detectedEntities` 变化时，本地状态同步更新：
```typescript
useEffect(() => {
  setLocalEntities(detectedEntities);
}, [detectedEntities]);
```

### 3. 去重处理
使用 Map 确保实体列表唯一：
```typescript
const uniqueEntities = Array.from(
  new Map(localEntities.map(e => [e.text, e])).values()
);
```

### 4. 持久化删除
父组件使用 Set 记录已删除的实体，确保在对话框关闭重开后，删除状态保持：
```typescript
const [removedEntities, setRemovedEntities] = useState<Set<string>>(new Set());
```

## 用户体验优化

### 视觉反馈
- ✅ 删除按钮默认隐藏，减少视觉干扰
- ✅ 悬停时显示，提供清晰的操作提示
- ✅ 红色按钮，明确表示删除操作
- ✅ 圆形设计，符合常见的关闭按钮样式

### 交互优化
- ✅ 点击区域分离：Badge 主体和删除按钮互不干扰
- ✅ 即时反馈：删除后立即从列表中移除
- ✅ 无需确认：快速操作，提高效率
- ✅ 可恢复：关闭对话框重新打开可恢复（如需要）

### 性能考虑
- ✅ 使用 Set 存储删除的实体，O(1) 查找性能
- ✅ 过滤操作在渲染时进行，不影响原始数据
- ✅ 状态更新使用函数式更新，避免闭包问题

## 未来改进方向

1. **撤销功能**
   - 添加"撤销删除"按钮
   - 显示最近删除的实体列表
   - 支持批量恢复

2. **删除原因标注**
   - 记录删除原因（误识别/不需要脱敏/其他）
   - 用于改进 NER 模型

3. **批量删除**
   - 支持按类型批量删除（如删除所有"地址"类型）
   - 支持正则表达式匹配删除

4. **持久化配置**
   - 保存用户的删除偏好
   - 自动过滤常见的误识别模式

## 版本信息

- **功能添加版本**: v0.1.34+
- **相关组件**:
  - `src/components/file/FindReplaceDialog.tsx`
  - `src/components/file/MaskingPreviewDialog.tsx`
- **依赖**: lucide-react (X 图标)

## 测试建议

### 功能测试
1. ✅ 删除单个实体
2. ✅ 删除后实体不再显示
3. ✅ 删除按钮不触发添加操作
4. ✅ 悬停显示/隐藏效果正常
5. ✅ 删除后关闭对话框，重新打开验证状态

### 边界测试
1. ✅ 删除所有实体后显示空状态提示
2. ✅ 快速连续删除多个实体
3. ✅ 删除后添加替换条目
4. ✅ 删除已添加到替换列表的实体

### 兼容性测试
1. ✅ 不同浏览器的悬停效果
2. ✅ 触摸屏设备的操作体验
3. ✅ 键盘导航支持（可选）

---

**注意**: 此功能仅影响当前会话的显示，不会修改原始的 NER 识别结果。如需永久改进识别准确性，请通过规则配置或模型训练来优化。
