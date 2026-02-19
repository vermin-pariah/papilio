FROM ubuntu:24.04

# 避免交互式配置提示
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    ffmpeg \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 直接复制宿主机编译好的 release 二进制文件
COPY target/release/papilio-server /app/papilio-server
# 复制迁移脚本以支持自动迁移
COPY papilio-core/migrations /app/papilio-core/migrations

EXPOSE 3000

# 增加执行权限并更新链接库缓存
RUN chmod +x /app/papilio-server && ldconfig

CMD ["/app/papilio-server"]
