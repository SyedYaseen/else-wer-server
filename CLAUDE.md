# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Else-wer is a audiobook player app using react native that for self hosted audiobook server.
pwa: /home/loop/p/else-wer/else-wer-server/src/ui
Server (This): /home/loop/p/else-wer/else-wer-server
App: /home/loop/p/else-wer/else-wer-app ( dont view or make changes here fo now)
WebUI (potentially go away/ reworked) : A custom file organizer: /home/loop/p/else-wer/else-wer-web

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

## Coding Guidelines

- Ask for clarification if requirements are ambiguous.
- Minimize token usage.
- Prefer the smallest possible change that solves the task.
- Avoid refactoring unrelated code.
- If solving the task requires widespread changes, explain why before proceeding.
- Reuse existing project patterns whenever possible.
- When Graphify identifies the relevant files, read only those files unless additional context is required.
- Never commit code since I want to review it first.
- Download, save, use relevant skills when deemed necessary.

## Investigation

Before editing:

- Search for existing implementations of the same pattern.
- Prefer modifying existing code over introducing new abstractions.
- Preserve naming conventions and project architecture.

## Before/ After any execution - Plan/ Auto/ Review

- Use this file/folder ./todo/CLAUDE_PROGRESS.md as memory across session for plans, marking progress and as checklist. Create additional priority based numbered file if the task is long in same folder.
- Mark the todo (Top of file) and done sections (Bottom of file) using byte position/ line number "seekers" to be efficient when traversing file, instead rereading the whole file.
- Split work into phases and save it to file so work can be continued across sessions.
- Group similar work together that need to done using the same context, so as not to reread same portions of code over and over unnecesarily.
- If there are out of scope items/ deferred for later items when doing a particular task add enough plan/notes to continue in a different session to be efficient with tokens.

## Validation
After making changes:
- Build or run the smallest relevant test.
- Fix compilation or lint errors introduced by your changes.
- Do not modify unrelated files.
