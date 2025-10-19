# Rust Web Frameworks

This document provides a brief overview of three popular Rust web frameworks for building frontend applications.

## Leptos

Leptos is a modern, full-stack Rust web framework that emphasizes fine-grained reactivity and performance. It uses a signals-based reactivity system that minimizes unnecessary re-renders and DOM updates. Leptos supports both server-side rendering (SSR) and client-side rendering, making it suitable for building SEO-friendly web applications. The framework compiles to WebAssembly for the client side and can run on the server using native Rust, enabling code sharing between frontend and backend.

**Key Features:**

- Fine-grained reactivity with signals
- Server-side rendering (SSR) support
- Isomorphic rendering (code runs on both client and server)
- Small bundle sizes
- Built-in routing and state management

## Yew

Yew is a component-based framework for building web applications in Rust, inspired by React and Elm. It was one of the first major Rust frontend frameworks and has a mature ecosystem. Yew uses a virtual DOM approach for efficient UI updates and supports component-based architecture with props and callbacks. The framework compiles to WebAssembly and runs entirely in the browser, making it ideal for single-page applications (SPAs).

**Key Features:**

- Component-based architecture similar to React
- Virtual DOM for efficient rendering
- Strong typing and compile-time guarantees
- Concurrent rendering support
- Rich ecosystem with many community libraries

## Dioxus

Dioxus is a portable, performant framework for building cross-platform user interfaces in Rust. It takes inspiration from React with its hooks-based API and virtual DOM, but extends beyond the web to support desktop (via native webviews), mobile, terminal UIs, and more. Dioxus emphasizes developer experience with features like hot reloading and a familiar React-like syntax. The framework is designed to let you write your UI once and deploy it to multiple platforms.

**Key Features:**

- Cross-platform support (web, desktop, mobile, TUI)
- React-like hooks API
- Virtual DOM with async rendering
- Built-in state management
- Hot reloading for rapid development
- Server-side rendering support

## Comparison

| Feature | Leptos | Yew | Dioxus |
|---------|--------|-----|--------|
| **Reactivity** | Fine-grained signals | Virtual DOM | Virtual DOM |
| **SSR Support** | Yes | Limited | Yes |
| **Target Platforms** | Web (client/server) | Web (client-side) | Web, desktop, mobile, TUI |
| **API Style** | Signals-based | Component-based | Hooks-based (React-like) |
| **Maturity** | Newer | Mature | Growing |

## Which to Choose?

- **Choose Leptos** if you need fine-grained reactivity, excellent SSR support, and minimal bundle sizes for web-focused applications.
- **Choose Yew** if you want a mature, stable framework for client-side web applications with a large ecosystem.
- **Choose Dioxus** if you need cross-platform support or prefer a React-like development experience with hooks.
