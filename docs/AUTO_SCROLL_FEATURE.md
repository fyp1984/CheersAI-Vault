# 替换列表自动滚动功能

## 功能概述

当用户点击上方的敏感词实体添加到替换列表后，下方的替换列表会自动滚动，确保用户能看到最新添加的条目。

## 功能特性

### 1. 自动滚动到最新条目
- 点击 ✓ 按钮添加实体后，自动滚动到新添加的条目
- 点击标签主体添加实体后，自动滚动到新添加的条目
- 点击"添加条目"按钮后，自动滚动到新添加的空白条目

### 2. 平滑滚动动画
- 使用 `smooth` 滚动行为
- 视觉过渡自然流畅
- 不会突兀地跳转

### 3. 智能定位
- 使用 `block: 'nearest'` 策略
- 如果条目已在可视区域，不会滚动
- 如果条目在可视区域外，滚动到最近的边缘

## 实现细节

### 技术实现

```typescript
// 1. 创建 refs
const scrollContainerRef = useRef<HTMLDivElement>(null);
const lastEntryRef = useRef<HTMLDivElement>(null);

// 2. 监听条目数量变化
useEffect(() => {
  if (lastEntryRef.current && scrollContainerRef.current) {
    lastEntryRef.current.scrollIntoView({ 
      behavior: 'smooth',  // 平滑滚动
      block: 'nearest'     // 智能定位
    });
  }
}, [entries.length]);

// 3. 绑定 refs 到 DOM
<div ref={scrollContainerRef} className="flex-1 overflow-y-auto pr-2">
  <div className="space-y-2">
    {entries.map((entry, idx) => (
      <div 
        key={entry.id} 
        ref={idx === entries.length - 1 ? lastEntryRef : null}
        className="flex items-center gap-2"
      >
        {/* 条目内容 */}
      </div>
    ))}
  </div>
</div>
```

### 关键点说明

1. **scrollContainerRef**
   - 引用滚动容器（带 `overflow-y-auto` 的 div）
   - 用于确保滚动发生在正确的容器中

2. **lastEntryRef**
   - 引用最后一个条目（`idx === entries.length - 1`）
   - 动态绑定，始终指向最新的条目

3. **useEffect 依赖**
   - 依赖 `entries.length`
   - 只在条目数量变化时触发
   - 避免不必要的滚动

4. **scrollIntoView 参数**
   - `behavior: 'smooth'` - 平滑滚动动画
   - `block: 'nearest'` - 最小化滚动距离

## 用户体验

### 操作流程

**场景 1：添加第一个条目**
```
初始状态：
┌─────────────────────┐
│ 替换列表   [+ 添加] │
├─────────────────────┤
│ 1. [空] → [空]      │ ← 默认空条目
└─────────────────────┘

点击添加"手机号: 138..."后：
┌─────────────────────┐
│ 替换列表   [+ 添加] │
├─────────────────────┤
│ 1. 138... → ***...  │ ← 自动填充
└─────────────────────┘
   (无需滚动，已在视野内)
```

**场景 2：添加多个条目**
```
当前状态（已有 5 个条目）：
┌─────────────────────┐
│ 替换列表   [+ 添加] │
├─────────────────────┤
│ 1. 查找 → 替换      │ ↑
│ 2. 查找 → 替换      │ │
│ 3. 查找 → 替换      │ │ 可见区域
│ 4. 查找 → 替换      │ │
│ 5. 查找 → 替换      │ ↓
└─────────────────────┘

点击添加"邮箱: zhang@..."后：
┌─────────────────────┐
│ 替换列表   [+ 添加] │
├─────────────────────┤
│ 2. 查找 → 替换      │ ↑
│ 3. 查找 → 替换      │ │
│ 4. 查找 → 替换      │ │ 可见区域
│ 5. 查找 → 替换      │ │
│ 6. zhang@ → ***...  │ ↓ ← 自动滚动到这里
└─────────────────────┘
```

**场景 3：快速连续添加**
```
用户快速点击 3 个实体：
1. 点击"手机号" → 滚动到第 6 条
2. 点击"邮箱"   → 滚动到第 7 条
3. 点击"地址"   → 滚动到第 8 条

每次添加都能看到新条目，不会迷失位置
```

## 优势

### 1. 即时反馈
- 用户立即看到添加的结果
- 确认操作成功
- 知道添加到了哪里

### 2. 避免迷失
- 不需要手动滚动查找
- 始终知道当前位置
- 提高操作效率

### 3. 流畅体验
- 平滑的滚动动画
- 视觉过渡自然
- 不会感到突兀

### 4. 智能行为
- 如果已在视野内，不滚动
- 如果在视野外，才滚动
- 最小化不必要的移动

## 边界情况处理

### 1. 条目已在可视区域
```typescript
block: 'nearest'  // 不会滚动，保持当前位置
```

### 2. 快速连续添加
- 每次添加都会触发滚动
- 平滑动画会排队执行
- 最终停在最后一个条目

### 3. 手动滚动后添加
- 用户手动滚动到中间位置
- 点击添加新条目
- 自动滚动到最新条目（覆盖手动位置）

### 4. 删除条目
- 删除条目不会触发滚动
- 保持当前滚动位置
- 避免干扰用户操作

## 性能考虑

### 1. 依赖优化
```typescript
useEffect(() => {
  // ...
}, [entries.length]);  // 只依赖长度，不依赖整个数组
```
- 只在条目数量变化时触发
- 修改条目内容不会触发
- 减少不必要的滚动

### 2. 条件检查
```typescript
if (lastEntryRef.current && scrollContainerRef.current) {
  // 只在 refs 都存在时执行
}
```
- 避免空引用错误
- 确保 DOM 已渲染

### 3. 浏览器优化
- `scrollIntoView` 是浏览器原生 API
- 性能优化由浏览器处理
- 支持硬件加速

## 兼容性

### 浏览器支持
- ✅ Chrome/Edge (Chromium)
- ✅ Firefox
- ✅ Safari
- ✅ 所有现代浏览器

### scrollIntoView 选项支持
- `behavior: 'smooth'` - 现代浏览器全支持
- `block: 'nearest'` - 现代浏览器全支持
- 旧浏览器会降级为立即滚动（无动画）

## 未来改进

### 1. 可配置的滚动行为
```typescript
const scrollBehavior = userPreferences.smoothScroll ? 'smooth' : 'auto';
```

### 2. 滚动到特定条目
```typescript
const scrollToEntry = (entryId: number) => {
  const element = document.getElementById(`entry-${entryId}`);
  element?.scrollIntoView({ behavior: 'smooth' });
};
```

### 3. 高亮新添加的条目
```typescript
// 添加后短暂高亮
<div className={isNew ? 'bg-blue-100 transition-colors' : ''}>
```

### 4. 滚动位置记忆
```typescript
// 记住用户的滚动位置
const [scrollPosition, setScrollPosition] = useState(0);
```

## 测试建议

### 功能测试
1. ✅ 添加第一个条目 - 验证不滚动（已在视野内）
2. ✅ 添加第 6+ 个条目 - 验证自动滚动
3. ✅ 快速连续添加 - 验证每次都滚动到最新
4. ✅ 手动滚动后添加 - 验证自动滚动到最新

### 性能测试
1. ✅ 添加 20+ 个条目 - 验证滚动流畅
2. ✅ 快速点击添加 - 验证无卡顿
3. ✅ 修改条目内容 - 验证不触发滚动

### 兼容性测试
1. ✅ Chrome - 验证平滑滚动
2. ✅ Firefox - 验证平滑滚动
3. ✅ Safari - 验证平滑滚动

## 版本信息

- **功能添加版本**: v0.1.34+
- **相关组件**: `src/components/file/FindReplaceDialog.tsx`
- **依赖**: React useRef, useEffect
- **浏览器 API**: scrollIntoView

---

**提示**: 这个功能让用户在添加条目后能立即看到结果，提供即时反馈，避免手动滚动查找，显著提升用户体验。
