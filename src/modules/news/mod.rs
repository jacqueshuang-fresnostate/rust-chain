//! news bounded context module root.
//!
//! 新闻公告限界上下文的对外只读侧：把后台维护的新闻按通用、行情、产品、系统、活动五类分发给终端用户。
//! 每条新闻带横幅图与列表小图标，正文以多语言 JSON 文档存放，内含版本号、默认语言与按语言和国家区分的内容项，
//! 服务端原样透出全部语言项，历史语言版本不会因当前请求语言而被裁剪丢失。
//! 新闻状态在后台侧经历草稿、发布与归档，本上下文只读取已发布记录，草稿与归档内容不会出现在任何公开响应中。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod routes;
