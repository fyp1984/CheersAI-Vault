FROM nginx:1.29-alpine

COPY docker/web-nginx.conf /etc/nginx/conf.d/default.conf
COPY dist /usr/share/nginx/html

EXPOSE 5173
