# Snowbros Atlas — Master Project Context

## Mission

We are building **Snowbros Atlas**, an engineering intelligence platform.

This is **NOT** another AI code reviewer.

This is **NOT** another linter.

This is **NOT** another chatbot.

Its primary goal is to **deterministically understand an entire software project** and detect engineering problems with extremely high confidence.

AI is optional and never responsible for deciding whether an issue exists.

The engine must always produce the same result for the same codebase.

---

# Core Philosophy

The project should behave like a compiler.

Compiler:

- Detects syntax problems.

Snowbros Atlas:

- Detects engineering problems.

It should answer questions such as:

- Why is this page slow?
- Which component causes unnecessary rerenders?
- Which API is dead?
- Which database query is inefficient?
- Which architecture decision is dangerous?
- Which code will break after changing one interface?
- Why did performance decrease after yesterday's commit?
- Which routes became dynamic?
- Which dependencies create hidden coupling?
- Which middleware causes unreachable code?
- Which environment variables are unused?
- Which server actions introduce security risks?

The tool must explain root causes, not only symptoms.

---

# Primary Design Principles

1. Deterministic
2. Extremely fast
3. Local-first
4. Offline capable
5. Framework-aware
6. Architecture-aware
7. Incremental
8. Explainable
9. Extensible
10. Plugin-based

Never rely on cloud inference for core functionality.

---

# Primary Goal

Given any repository, build a complete semantic understanding of the project and detect engineering issues that existing tools typically miss.

---

# Long-Term Vision

Become the engineering operating system for developers.

Instead of using:

- ESLint
- Sonar
- Lighthouse
- Bundle Analyzer
- Security Scanner
- Architecture Visualizer
- Circular Dependency Checker

Developers use one engine.

---

# Product Architecture

Snowbros Atlas consists of independent modules.

scanner/
parser/
semantic-graph/
framework-detectors/
rule-engine/
report-engine/
cache-engine/
watch-engine/
plugin-system/
cli/
vscode-extension/
dashboard/
sdk/

Each module must be replaceable.

---

# Technology Stack

Core Language:
Rust

Reason:
Performance
Safety
Cross-platform
Concurrency
Single executable

Frontend:
Next.js
React
TypeScript

VS Code:
TypeScript

Storage:
PostgreSQL

Configuration:
TOML

Serialization:
Serde JSON

---

# Parsing

Never build parsers manually.

Use existing libraries.

JavaScript:
oxc

TypeScript:
TypeScript Compiler API

Tree Parsing:
Tree-sitter

Graph:
petgraph

Parallelism:
rayon

File Watching:
notify

CLI:
clap

Logging:
tracing

---

# Framework Detection

Automatically detect:

Next.js

React

Vue

Angular

Svelte

Solid

Node

Express

NestJS

Laravel

Django

Supabase

Prisma

Drizzle

Astro

Nuxt

Detection should happen automatically through:

package.json

config files

folder structure

known dependencies

No manual configuration.

---

# Build a Semantic Graph

This is the heart of the project.

The graph should understand:

Files

Directories

Components

Hooks

Functions

Classes

Interfaces

Types

Imports

Exports

Dependencies

Routes

Layouts

Middleware

API endpoints

Database tables

RPCs

Policies

Schemas

Environment variables

Assets

Images

Fonts

CSS

Authentication

Caching

Deployment

Testing

Every node should know its relationships.

This graph becomes the foundation for every analyzer.

---

# Analyzer Categories

React Analyzer

Next.js Analyzer

TypeScript Analyzer

Node Analyzer

Database Analyzer

Performance Analyzer

Security Analyzer

Accessibility Analyzer

SEO Analyzer

Architecture Analyzer

Dependency Analyzer

Testing Analyzer

Caching Analyzer

Routing Analyzer

Authentication Analyzer

Deployment Analyzer

Environment Analyzer

API Analyzer

State Management Analyzer

Memory Analyzer

Bundle Analyzer

Every analyzer should be isolated.

---

# Rule Engine

Every rule should contain:

Rule ID

Title

Description

Framework

Category

Severity

Confidence

Detection Logic

Evidence

Impact

Suggested Fix

Documentation Link

False Positive Notes

Rules should never be hardcoded throughout the engine.

Use a plugin architecture.

---

# Confidence

Every issue receives a confidence score.

100%

Likely

Possible

Unknown

Never exaggerate confidence.

---

# Severity

Critical

High

Medium

Low

Info

---

# Example Findings

Dynamic rendering because cookies()

Infinite render loops

Server component importing client-only logic incorrectly

Unused APIs

Circular dependencies

Duplicate business logic

Missing indexes

Unreachable routes

Memory leaks

Race conditions

Large bundles

Hydration mismatch risks

Cache invalidation issues

Dead environment variables

Unprotected admin routes

Authentication bypass possibilities

SEO metadata inconsistencies

Broken canonical URLs

Accessibility issues

Improper Suspense usage

N+1 query patterns

Large React components

Excessive prop drilling

Database schema inconsistencies

Weak architectural boundaries

Unused database tables

Unused assets

Unused dependencies

Potential deployment problems

---

# Output Format

Everything should become structured JSON.

Never mix analysis logic with presentation.

CLI

VS Code

Dashboard

API

should all consume the same data.

---

# CLI

Examples:

sb inspect

sb inspect --json

sb inspect --html

sb inspect --markdown

sb inspect --ci

sb graph

sb doctor

sb explain RULE_ID

---

# VS Code Extension

The extension must remain lightweight.

It should:

Receive JSON

Display diagnostics

Show explanations

Navigate to findings

Never duplicate analysis logic.

---

# Dashboard

Later phases include:

Repository history

Trend analysis

Performance tracking

Architecture score evolution

Team analytics

CI integration

Organization dashboards

---

# Plugin System

Support plugins.

Examples:

React Plugin

Next Plugin

Supabase Plugin

Prisma Plugin

Laravel Plugin

Django Plugin

Vue Plugin

Angular Plugin

Every plugin provides:

Rules

Metadata

Configuration

Documentation

Version

---

# Performance Goals

Cold scan:

<5 seconds on medium repositories.

Incremental scan:

Sub-second whenever possible.

Use:

Incremental parsing

Graph caching

Parallel execution

File watching

Never rescan the entire project unless necessary.

---

# False Positives

Accuracy is more important than quantity.

Prefer missing one issue over reporting ten incorrect ones.

Every finding should include evidence.

---

# Root Cause Analysis

Never flood users with duplicate findings.

Instead of:

10 warnings

Identify:

1 architectural root cause

Show downstream effects.

---

# Project Scoring

Generate category scores:

Architecture

Performance

Security

Accessibility

SEO

Database

Testing

Maintainability

Reliability

Overall Health

Scores must be explainable.

---

# Reports

Support:

Terminal

JSON

HTML

Markdown

SARIF

Future PDF export.

---

# AI

AI is optional.

AI should never detect issues.

AI only:

Explains findings

Suggests fixes

Generates patches

Answers user questions

The deterministic engine always remains the source of truth.

---

# Development Roadmap

Phase 1

Scanner

Framework Detection

Parser

Semantic Graph

CLI

20–50 elite rules

Phase 2

200+ rules

VS Code extension

Incremental scanning

Plugin system

Phase 3

Dashboard

CI integration

Historical analysis

Enterprise reporting

Phase 4

Optional AI assistant

Patch generation

Custom organization rules

Multi-repository intelligence

---

# Non-Goals

Do not become another autocomplete tool.

Do not become another chatbot.

Do not become another formatter.

Do not replace existing compilers.

Focus on engineering intelligence.

---

# Coding Standards

Modular architecture

SOLID principles

Strong typing

High test coverage

Benchmark performance regularly

Document every public module

No unnecessary dependencies

No hidden magic

Everything should be deterministic and explainable.

---

# Success Criteria

The project succeeds if developers say:

"I already had ESLint, TypeScript, and my IDE—but Snowbros Atlas found issues none of them caught, explained the root cause, and did it in seconds."

That is the standard every design and implementation decision should be measured against.
