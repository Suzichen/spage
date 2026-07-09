---
title: Markdown Quick Reference
date: 2025-01-01 12:00:00
tags: [blog-system, markdown]
categories: [Project]
preview: This is a basic Markdown syntax example for the Spage blog.
---

## Spage Basic Syntax Manual

### 1. Italics and Bold

Use * and ** to format as italics and bold.

Example:

This is *italics*, and this is **bold**.

### 2. Heading Levels

Use `#` to format as an H1 heading, and `##` for an H2 heading.

Example:

```
# This is an H1 heading

## This is an H2 heading

### This is an H3 heading
```

### 3. Links

Use `[description](link)` to add a link to text.

Example:

This is a link to [my blog](http://s-blog.suzichen.me).

### 4. Unordered Lists

Use `*`, `+`, `-` to format as unordered lists.

Example:

- Unordered list item 1
- Unordered list item 2
- Unordered list item 3

### 5. Ordered Lists

Use numbers and a dot to format as ordered lists.

Example:

1. Ordered list item 1
2. Ordered list item 2
3. Ordered list item 3

### 6. Blockquotes

Use `>` to format as blockquotes.

Example:

> Not even a prairie fire can destroy the grass, it grows again when the spring breeze blows.

### 7. Inline Code

Use \`code` to format as inline code.

Example:

Let's talk about `html`.

### 8. Code Blocks

Use a set of \``` to format as code blocks.

Example:
```
func:
    This is a code block.
```

> You can also use four indentation spaces to express it.

### 9. Images

Use \!\[description](image link) to insert an image.

Example:

![AI's impression of me](https://img.suzichen.me/other/blog-suzichen-me-chatGPT.png)

## Spage Advanced Syntax Manual

### 1. Strikethrough

Use a set of `~~` to format as a strikethrough.

~~This is incorrect text.~~

### 2. Footnotes

Use `[^keyword]` to format as a footnote.

This is an example of a footnote[^footnote].

This is an example of a second footnote[^footnote2].

### 3. Enhanced Code Blocks

Supports syntax highlighting and line numbers for 41 programming languages.

Normal Example:

```
$ sudo apt-get install vim-gnome
```

Python Example:

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

JavaScript Example:

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

### 4. Tables

Use the following syntax to create a table:
```
| Item       | Price   |  Quantity  |
| --------   | -----:  | :----:    |
| Computer   | $1600   |   5     |
| Phone      |   $12   |   12   |
| Cable       |   $1    |  234  |
```

| Item        | Price   |  Quantity  |
| --------   | -----:  | :----:  |
| Computer   | $1600 |   5     |
| Phone      |   $12   |   12   |
| Cable       |    $1    |  234  |

### 5. Task Lists

Use list syntax with [ ] or [x] (incomplete or complete) items to write a task list. It also supports nested sublists and mixed Markdown syntax, for example:

    - [ ] **Spage Development**
        - [ ] Improve SEO functionality to generate different SEO files for different languages
        - [x] Add Todo list functionality [syntax reference](https://github.com/blog/1375-task-lists-in-gfm-issues-pulls-comments)
        - [x] Improve Engine functionality
            - [x] Enhance photo compression performance
            - [x] Optimize Log output to support [Swritor](https://github.com/Suzichen/swritor) display
    - [ ] **July Travel Preparation**
        - [ ] Prepare items needed for the cruise
        - [ ] Browse items in Japanese duty-free shops
        - [x] Buy July 1st ticket for Sapphire Princess
        
Correspondingly displayed as the following Task list:

- [ ] **Spage Development**
    - [ ] Improve SEO functionality to generate different SEO files for different languages
    - [x] Add Todo list functionality [syntax reference](https://github.com/blog/1375-task-lists-in-gfm-issues-pulls-comments)
    - [x] Improve Engine functionality
        - [x] Enhance photo compression performance
        - [x] Optimize Log output to support [Swritor](https://github.com/Suzichen/swritor) display
- [ ] **July Travel Preparation**
    - [ ] Prepare items needed for the cruise
    - [ ] Browse items in Japanese duty-free shops
    - [x] Buy July 1st ticket for Sapphire Princess
        
        
[^footnote]: This is the **text** of a *footnote*.

[^footnote2]: This is the **text** of another *footnote*.
