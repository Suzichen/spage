---
title: Markdown 简明语法手册
date: 2025-01-01 12:00:00
tags: [blog-system, markdown]
categories: [Project]
preview: 这是一篇适用于 Spage 博客的基础 Markdown 语法示例。
---

## Spage 简明语法手册

### 1. 斜体和粗体

使用 * 和 ** 表示斜体和粗体。

示例：

这是 *斜体*，这是 **粗体**。

### 2. 分级标题

使用 `#` 表示一级标题，使用 `##` 表示二级标题。

示例：

```
# 这是一个一级标题

## 这是一个二级标题

### 这是一个三级标题
```

### 3. 链接

使用 `[描述](链接地址)` 为文字增加链接。

示例：

这是去往 [本人博客](http://s-blog.suzichen.me) 的链接。

### 4. 无序列表

使用 `*`，`+`，`-` 表示无序列表。

示例：

- 无序列表项 一
- 无序列表项 二
- 无序列表项 三

### 5. 有序列表

使用数字和点表示有序列表。

示例：

1. 有序列表项 一
2. 有序列表项 二
3. 有序列表项 三

### 6. 文字引用

使用 `>` 表示文字引用。

示例：

> 野火烧不尽，春风吹又生。

### 7. 行内代码块

使用 \`代码` 表示行内代码块。

示例：

让我们聊聊 `html`。

### 8.  代码块

使用一组 \``` 表示代码块。

示例：
```
func:
    这是一个代码块。
```

> 你也可以使用 四个缩进空格 来表达

### 9.  插入图像

使用 \!\[描述](图片链接地址) 插入图像。

示例：

![我的AI印象](https://img.suzichen.me/other/blog-suzichen-me-chatGPT.png)

## Spage 高阶语法手册

### 1. 删除线

使用一组 `~~` 表示删除线。

~~这是一段错误的文本。~~

### 2. 注脚

使用 `[^keyword]` 表示注脚。

这是一个注脚[^footnote]的样例。

这是第二个注脚[^footnote2]的样例。

### 3. 加强的代码块

支持四十一种编程语言的语法高亮的显示，行号显示。

普通示例：

```
$ sudo apt-get install vim-gnome
```

Python 示例：

``` python
@requires_authorization
def somefunc(param1='', param2=0):
    '''A docstring'''
    if param1 > param2: # interesting
        print('Greater')
    return (param2 - param1 + 1) or None

class SomeClass:
    pass

>>> message = '''interpreter
... prompt'''
```

JavaScript 示例：

``` javascript
/**
* nth element in the fibonacci series.
* @param n >= 0
* @return the nth element, >= 0.
*/
function fib(n) {
  var a = 1, b = 1;
  var tmp;
  while (--n >= 0) {
    tmp = a;
    a += b;
    b = tmp;
  }
  return a;
}

document.write(fib(10));
```

### 4. 表格支持

使用
```
| 项目        | 价格   |  数量  |
| --------   | -----:  | :----:  |
| 计算机     | $1600 |   5     |
| 手机        |   $12   |   12   |
| 管线        |    $1    |  234  |
```
这样的语法来表示表格: 

| 项目        | 价格   |  数量  |
| --------   | -----:  | :----:  |
| 计算机     | $1600 |   5     |
| 手机        |   $12   |   12   |
| 管线        |    $1    |  234  |

### 5. 待办事宜 Todo 列表

使用带有 [ ] 或 [x] （未完成或已完成）项的列表语法撰写一个待办事宜列表，并且支持子列表嵌套以及混用Markdown语法，例如：

    - [ ] **Spage 开发**
        - [ ] 改进 SEO 功能，使不同语言产出不同的SEO文件
        - [x] 新增Todo列表功能 [语法参考](https://github.com/blog/1375-task-lists-in-gfm-issues-pulls-comments)
        - [x] 改进 Engine 功能
            - [x] 提升照片压缩性能
            - [x] 优化Log输出以支持 [Swritor](https://github.com/Suzichen/swritor)的展示
    - [ ] **七月旅行准备**
        - [ ] 准备邮轮上需要携带的物品
        - [ ] 浏览日本免税店的物品
        - [x] 购买蓝宝石公主号七月一日的船票
        
对应显示如下待办事宜 Todo 列表：

- [ ] **Spage 开发**
    - [ ] 改进 SEO 功能，使不同语言产出不同的SEO文件
    - [x] 新增Todo列表功能 [语法参考](https://github.com/blog/1375-task-lists-in-gfm-issues-pulls-comments)
    - [x] 改进 Engine 功能
        - [x] 提升照片压缩性能
        - [x] 优化Log输出以支持 [Swritor](https://github.com/Suzichen/swritor)的展示
- [ ] **七月旅行准备**
    - [ ] 准备邮轮上需要携带的物品
    - [ ] 浏览日本免税店的物品
    - [x] 购买蓝宝石公主号七月一日的船票
        
        
[^footnote]: 这是一个 *注脚* 的 **文本**。

[^footnote2]: 这是另一个 *注脚* 的 **文本**。
