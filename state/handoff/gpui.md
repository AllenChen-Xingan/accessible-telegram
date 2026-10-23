# 现状页 · gpui

- 手上：命门 1、2 —— GPUI 无障碍接口核查（交代 `_tmp/20261023-dispatch/gpui.md`）
- 做到哪：**完成**。五项核查全部有结论，结果在 `docs/research/001_stack/findings-gpui.md`（约 670 汉字）
  1. 列表项进树：原生 ✅
  2. 程序移焦点 + VoiceOver 跟随：原生 ✅（目标项须在渲染区内；另有 active_descendant）
  3. 虚拟化列表屏幕外项：没有 ❌（不进树；滚动类读屏动作不处理）
  4. 主动播报：部分（无专用接口，但可经公开出口设 AccessKit `Live`，accesskit_macos 会发 AnnouncementRequested）
  5. 导出树：部分（`debug_a11y_tree_json` 有，但 GPUI 单元测试平台不激活无障碍，测试里拿不到）
  - 判断：首选「改造」（本项目薄壳，不 fork），备选「直接用」
- 取材：`resources/zed`（浅克隆，不入库）；accesskit_macos 0.26.3 的 `event.rs` 经 gh api 读取，临时放在会话 scratchpad，未入项目
- 没做：未编译、未实测播报；上游「live region / 列表」相关 PR 的 gh 搜索返回空，未深挖
- 下一步：等大总管指示
