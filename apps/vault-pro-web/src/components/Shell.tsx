import { FileUp, ListChecks, Lock, Server } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";
import { runtimeBaseUrl } from "../api/client";

const navItems = [
  { to: "/submit", label: "批量提交", Icon: FileUp },
  { to: "/restore", label: "反脱敏", Icon: Lock },
  { to: "/batches", label: "处理日志", Icon: ListChecks },
];

export function Shell() {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brand-mark" src="/logo.jpg" alt="CheersAI" />
          <div>
            <strong>CheersAI Vault Pro</strong>
            <span>企业版</span>
          </div>
        </div>
        <nav aria-label="企业工作台">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
            >
              <item.Icon className="nav-mark" aria-hidden="true" />
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="runtime-card">
          <Server className="runtime-icon" aria-hidden="true" />
          <div>
            <strong>本机 Runtime</strong>
            <span className="runtime-label">本机连接地址</span>
            <span className="runtime-url">{runtimeBaseUrl}</span>
          </div>
        </div>
      </aside>
      <main className="main-content">
        <header className="topbar">
          <span>企业文档脱敏工作台</span>
          <span className="scope-pill">TXT · Markdown · CSV · Excel · DOCX · PPT · PPTX · PDF</span>
        </header>
        <Outlet />
      </main>
    </div>
  );
}
