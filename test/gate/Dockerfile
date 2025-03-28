FROM ubuntu:20.04

WORKDIR /app
COPY . .
RUN chmod +x /app/gateway

EXPOSE 6188

ENTRYPOINT /app/gateway

