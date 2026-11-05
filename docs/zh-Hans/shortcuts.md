# 键盘快捷键

nnano 默认使用**桌面风格键位**。与桌面习惯不冲突的 nano 键位予以保留。

**记法：** `Ctrl` = `^` · `Alt` = `M-`。在 macOS 上 `Alt` 是 `Option`；`Cmd+*` 不会传送到应用。
*替代键* 一列是继承自 nano 的按键（如果存在）。

## 文件

| 操作 | 按键 | 替代键 |
|---|---|---|
| 保存 | `Ctrl+S` | — |
| 另存为 | `Alt+S` | — |
| 打开文件 | `Ctrl+O` | — |
| 从磁盘重新加载 | `Ctrl+R` | — |
| 在光标处插入文件 | `Alt+I` | `Ctrl+R`（nano） |
| 新建缓冲区 | `Ctrl+N` | — |
| 新建标签页 | `Ctrl+T` | — |
| 关闭缓冲区 | `Ctrl+W` | — |
| 退出 | `Ctrl+Q` | — |

## 编辑

| 操作 | 按键 | 替代键 |
|---|---|---|
| 撤销 | `Ctrl+Z` | `Alt+U` |
| 重做 | `Ctrl+Y` | `Alt+E` |
| 剪切 | `Ctrl+X` | — |
| 复制 | `Ctrl+C` | — |
| 粘贴 | `Ctrl+V` | `Ctrl+U` |
| 剪切整行 | `Ctrl+K` | — |
| 全选 | `Ctrl+A` | — |
| 开始标记 | `Alt+A` | `Alt+A` |
| 复制标记文本 | `Alt+6` | `Alt+6` |
| 删除字符 | `Ctrl+D` | — |
| 替换 | `Ctrl+\` | `Alt+R` |
| 段落对齐 | `Ctrl+J` | — |
| 执行外部命令 | `Alt+X` | — |

## 导航

| 操作 | 按键 | 替代键 |
|---|---|---|
| 跳转到行 | `Ctrl+G` | `Alt+G` |
| 行首 | `Home` | — |
| 行尾 | `End` | `Ctrl+E` |
| 第一行 | `Alt+\` | `Alt+\` |
| 最后一行 | `Alt+/` | `Alt+/` |
| 向左 / 向右一个词 | `Ctrl+←` / `Ctrl+→` | `Alt+←` / `Alt+→` |
| 向上翻页 | `PgUp` | `Alt+↑` |
| 向下翻页 | `PgDn` | `Alt+↓` |
| 后退一个字符 | `Ctrl+B` | `Ctrl+B` |

## 搜索

| 操作 | 按键 | 替代键 |
|---|---|---|
| 查找 | `Ctrl+F` | — |
| 查找下一个 | `F3` | `Alt+W` |
| 查找上一个 | `Shift+F3` | `Alt+B` |
| 切换大小写敏感（提示框内） | `Alt+C` | `Alt+C` |
| 切换正则表达式（提示框内） | `Alt+R` | `Alt+R` |

搜索行为与 nano 一致：**不是增量搜索**（按确认后才执行），默认按字面匹配，默认不区分大小写，到达文件末尾后回绕并提示 *已回绕搜索*。
`Alt+R` 是模式相关的：在提示框内切换正则，在提示框外打开替换。

## 界面

| 操作 | 按键 | 替代键 |
|---|---|---|
| 帮助 | `F1` | — |
| 设置菜单 | `Alt+,` | — |
| 命令面板 | `Ctrl+P` | — |
| 切换语法高亮 | `Alt+Y` | `Alt+Y` |
| 刷新屏幕 | `Ctrl+L` | `Ctrl+L` |

## 与 nano 的区别

| 原按键（nano） | 现在 |
|---|---|
| `Ctrl+O` 保存 | `Ctrl+S` 保存 |
| `Ctrl+X` 退出 | `Ctrl+Q` 退出 · `Ctrl+X` 剪切 |
| `Ctrl+W` 搜索 | `Ctrl+F` 搜索 · `Ctrl+W` 关闭缓冲区 |
| `Ctrl+C` 光标位置 | 状态栏 · `Ctrl+C` 复制 |
| `Ctrl+V` 向下翻页 | `PgDn` / `Alt+↓` · `Ctrl+V` 粘贴 |
| `Ctrl+Y` 向上翻页 | `PgUp` / `Alt+↑` · `Ctrl+Y` 重做 |
| `Ctrl+A` 行首 | `Home` · `Ctrl+A` 全选 |
| `Ctrl+Z` 挂起 | 已移除 · `Ctrl+Z` 撤销 |
| `Ctrl+R` 插入文件 | `Alt+I` · `Ctrl+R` 重新加载 |
| `Ctrl+N` / `Ctrl+P` 下一行 / 上一行 | 方向键 `↓` / `↑` · 新建缓冲区 / 命令面板 |
| `Ctrl+T` 执行命令 | `Alt+X` · `Ctrl+T` 新建标签页 |
| `Ctrl+G` 帮助 | `F1` · `Ctrl+G` 跳转到行 |

保持不变：`Ctrl+K`、`Ctrl+U`、`Ctrl+E`、`Ctrl+D`、`Ctrl+J`、`Ctrl+L`、`Ctrl+B`、`Ctrl+\`、`Alt+U`、`Alt+E`、`Alt+A`、`Alt+6`、`Alt+G`、`Alt+\`、`Alt+/`、`Alt+W`、`Alt+B`、`Alt+C`、`Alt+R`、`Alt+Y`。

## 终端限制

- `Ctrl+S` 和 `Ctrl+Q` 需要关闭 XON/XOFF 流控制，否则会被终端吞掉。
- `Enter`、`Tab`、`Backspace`、`Esc` 无法重新分配。
- `Ctrl+B` 是 tmux 的默认前缀；`Ctrl+A` 是 screen 的默认前缀。
- `Ctrl+Shift+字母` 在旧终端上不可靠：没有 `Ctrl+Shift+Z` 重做。
- 要用终端自带的方式选择文本，拖动时按住 `Shift`。
