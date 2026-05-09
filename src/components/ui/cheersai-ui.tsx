/**
 * CheersAI UI 组件库
 * 
 * 符合 CheersAI 产品 UI 规范 v1.0
 * 提供标准化的 UI 组件，确保全局一致性
 */

import React from 'react';
import { LucideIcon, CheckCircle, AlertTriangle, XCircle, Info } from 'lucide-react';

// ============ 按钮组件 ============

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'icon';
  size?: 'sm' | 'md' | 'lg';
  icon?: LucideIcon;
  loading?: boolean;
}

export function Button({ 
  variant = 'primary', 
  size = 'md',
  icon: Icon,
  loading,
  children, 
  className = '',
  disabled,
  ...props 
}: ButtonProps) {
  const baseStyles = 'inline-flex items-center justify-center gap-2 font-medium transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed';
  
  const variantStyles = {
    primary: 'bg-primary text-white hover:bg-primary-dark',
    secondary: 'bg-transparent text-gray-600 border border-gray-300 hover:bg-gray-100',
    icon: 'bg-transparent text-gray-600 hover:bg-gray-100',
  };
  
  const sizeStyles = {
    sm: variant === 'icon' ? 'w-8 h-8 p-1.5' : 'px-3 py-1.5 text-sm',
    md: variant === 'icon' ? 'w-10 h-10 p-2' : 'px-6 py-2.5 text-base',
    lg: variant === 'icon' ? 'w-12 h-12 p-3' : 'px-8 py-3 text-lg',
  };
  
  const radiusStyles = variant === 'icon' ? 'rounded-lg' : 'rounded-lg';
  
  return (
    <button
      className={`${baseStyles} ${variantStyles[variant]} ${sizeStyles[size]} ${radiusStyles} ${className}`}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? (
        <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
      ) : Icon ? (
        <Icon className="w-4 h-4" />
      ) : null}
      {children}
    </button>
  );
}

// ============ 徽章组件 ============

interface BadgeProps {
  variant?: 'success' | 'warning' | 'error' | 'info' | 'neutral';
  children: React.ReactNode;
  className?: string;
}

export function Badge({ variant = 'neutral', children, className = '' }: BadgeProps) {
  const variantStyles = {
    success: 'bg-success/10 text-success border-success/20',
    warning: 'bg-warning/10 text-warning border-warning/20',
    error: 'bg-error/10 text-error border-error/20',
    info: 'bg-info/10 text-info border-info/20',
    neutral: 'bg-gray-100 text-gray-700 border-gray-200',
  };
  
  return (
    <span className={`inline-flex items-center px-2 py-0.5 text-xs font-medium border rounded-full ${variantStyles[variant]} ${className}`}>
      {children}
    </span>
  );
}

// ============ 卡片组件 ============

interface CardProps {
  children: React.ReactNode;
  className?: string;
  hover?: boolean;
  selected?: boolean;
  onClick?: () => void;
}

export function Card({ children, className = '', hover = false, selected = false, onClick }: CardProps) {
  const baseStyles = 'bg-white border rounded-lg transition-all duration-200';
  const hoverStyles = hover ? 'hover:shadow-md hover:-translate-y-0.5 cursor-pointer' : '';
  const selectedStyles = selected ? 'border-primary bg-primary/5 shadow-md' : 'border-gray-200';
  
  return (
    <div 
      className={`${baseStyles} ${hoverStyles} ${selectedStyles} ${className}`}
      onClick={onClick}
    >
      {children}
    </div>
  );
}

// ============ 统计卡片组件 ============

interface StatCardProps {
  label: string;
  value: string | number;
  icon?: LucideIcon;
  trend?: {
    value: string;
    positive: boolean;
  };
  className?: string;
}

export function StatCard({ label, value, icon: Icon, trend, className = '' }: StatCardProps) {
  return (
    <Card className={`p-5 ${className}`}>
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <p className="text-sm text-gray-600 mb-1">{label}</p>
          <p className="text-3xl font-bold text-gray-900">{value}</p>
          {trend && (
            <p className={`text-sm mt-2 ${trend.positive ? 'text-success' : 'text-error'}`}>
              {trend.positive ? '↑' : '↓'} {trend.value}
            </p>
          )}
        </div>
        {Icon && (
          <div className="w-12 h-12 bg-primary/10 rounded-lg flex items-center justify-center">
            <Icon className="w-6 h-6 text-primary" />
          </div>
        )}
      </div>
    </Card>
  );
}

// ============ 输入框组件 ============

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helperText?: string;
}

export function Input({ label, error, helperText, className = '', ...props }: InputProps) {
  return (
    <div className="w-full">
      {label && (
        <label className="block text-sm font-medium text-gray-700 mb-1">
          {label}
        </label>
      )}
      <input
        className={`w-full px-4 py-2.5 border rounded-lg text-base text-gray-900 transition-all duration-200
          focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary
          disabled:bg-gray-50 disabled:text-gray-500 disabled:cursor-not-allowed
          ${error ? 'border-error' : 'border-gray-300'}
          ${className}`}
        {...props}
      />
      {error && (
        <p className="mt-1 text-sm text-error">{error}</p>
      )}
      {helperText && !error && (
        <p className="mt-1 text-sm text-gray-500">{helperText}</p>
      )}
    </div>
  );
}

// ============ 文本域组件 ============

interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  helperText?: string;
}

export function Textarea({ label, error, helperText, className = '', ...props }: TextareaProps) {
  return (
    <div className="w-full">
      {label && (
        <label className="block text-sm font-medium text-gray-700 mb-1">
          {label}
        </label>
      )}
      <textarea
        className={`w-full px-4 py-2.5 border rounded-lg text-base text-gray-900 transition-all duration-200 resize-none
          focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary
          disabled:bg-gray-50 disabled:text-gray-500 disabled:cursor-not-allowed
          ${error ? 'border-error' : 'border-gray-300'}
          ${className}`}
        {...props}
      />
      {error && (
        <p className="mt-1 text-sm text-error">{error}</p>
      )}
      {helperText && !error && (
        <p className="mt-1 text-sm text-gray-500">{helperText}</p>
      )}
    </div>
  );
}

// ============ 消息提示组件 ============

interface MessageProps {
  type: 'success' | 'warning' | 'error' | 'info';
  title?: string;
  children: React.ReactNode;
  onClose?: () => void;
  className?: string;
}

export function Message({ type, title, children, onClose, className = '' }: MessageProps) {
  const styles = {
    success: {
      bg: 'bg-success/5',
      border: 'border-success/20',
      text: 'text-success',
      icon: CheckCircle,
    },
    warning: {
      bg: 'bg-warning/10',
      border: 'border-warning/20',
      text: 'text-warning',
      icon: AlertTriangle,
    },
    error: {
      bg: 'bg-error/5',
      border: 'border-error/20',
      text: 'text-error',
      icon: XCircle,
    },
    info: {
      bg: 'bg-info/5',
      border: 'border-info/20',
      text: 'text-info',
      icon: Info,
    },
  };
  
  const style = styles[type];
  const IconComponent = style.icon;
  
  return (
    <div className={`${style.bg} border ${style.border} rounded-lg p-4 ${className}`}>
      <div className="flex items-start gap-3">
        <IconComponent className={`w-5 h-5 ${style.text} flex-shrink-0 mt-0.5`} />
        <div className="flex-1">
          {title && (
            <h4 className={`text-base font-semibold ${style.text} mb-1`}>{title}</h4>
          )}
          <div className="text-sm text-gray-700">{children}</div>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 transition-colors flex-shrink-0"
          >
            <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}

// ============ 加载状态组件 ============

interface LoadingProps {
  size?: 'sm' | 'md' | 'lg';
  text?: string;
  className?: string;
}

export function Loading({ size = 'md', text, className = '' }: LoadingProps) {
  const sizeStyles = {
    sm: 'w-4 h-4',
    md: 'w-8 h-8',
    lg: 'w-12 h-12',
  };
  
  return (
    <div className={`flex items-center justify-center gap-3 ${className}`}>
      <div className={`${sizeStyles[size]} border-2 border-primary border-t-transparent rounded-full animate-spin`} />
      {text && <span className="text-sm text-gray-600">{text}</span>}
    </div>
  );
}

// ============ 空状态组件 ============

interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

export function EmptyState({ icon: Icon, title, description, action, className = '' }: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center justify-center py-12 px-4 text-center ${className}`}>
      {Icon && (
        <div className="w-16 h-16 bg-gray-100 rounded-full flex items-center justify-center mb-4">
          <Icon className="w-8 h-8 text-gray-400" />
        </div>
      )}
      <h3 className="text-lg font-semibold text-gray-900 mb-2">{title}</h3>
      {description && (
        <p className="text-sm text-gray-600 mb-6 max-w-md">{description}</p>
      )}
      {action && (
        <Button onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}

// ============ 开关组件 ============

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  description?: string;
  disabled?: boolean;
  className?: string;
}

export function Switch({ checked, onChange, label, description, disabled, className = '' }: SwitchProps) {
  return (
    <div className={`flex items-center justify-between ${className}`}>
      {(label || description) && (
        <div className="flex-1">
          {label && <div className="text-base font-medium text-gray-900">{label}</div>}
          {description && <div className="text-sm text-gray-600 mt-0.5">{description}</div>}
        </div>
      )}
      <button
        type="button"
        onClick={() => !disabled && onChange(!checked)}
        disabled={disabled}
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-primary/20 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed ${
          checked ? 'bg-primary' : 'bg-gray-200'
        }`}
      >
        <span
          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ${
            checked ? 'translate-x-6' : 'translate-x-1'
          }`}
        />
      </button>
    </div>
  );
}
