# The offline host of the Quint obligations, which `just proofs` runs with `--network none`.
FROM docker.io/library/eclipse-temurin:27-jre@sha256:d6bfa738df69e8dec9061b3e26f2ae564d92ad169faa36086dcfa33f0a901d35 AS java

FROM docker.io/library/node:24-bookworm-slim@sha256:91acba91b842bec0bf7ea6f5e09c644c7ccad72c4996ed9616d861d1c7cd60d4
COPY --from=java /opt/java/openjdk /opt/java/openjdk
ENV PATH=/opt/java/openjdk/bin:/opt/quint/node_modules/.bin:$PATH
ENV QUINT_HOME=/opt/quint

# The lockfile pins Quint and every package it loads by integrity hash.
COPY package.json package-lock.json /opt/quint/
RUN cd /opt/quint && npm ci --ignore-scripts --no-audit --no-fund

# Quint compiles a model through Apalache and runs the model checker that Apalache bundles, with the modules the compiled model extends.
# The release lists the checksum in `sha256sum.txt`, and this file repeats it, so a replaced asset fails the build.
ARG APALACHE_VERSION=0.62.1
ARG APALACHE_SHA256=93fef2f381d30465eff906a2d6afa3358cb2c9adf6a8aad3bec0ded4f45432bb
ADD --checksum=sha256:${APALACHE_SHA256} https://github.com/apalache-mc/apalache/releases/download/v${APALACHE_VERSION}/apalache-${APALACHE_VERSION}.tgz /tmp/apalache.tgz
ADD https://github.com/apalache-mc/apalache/releases/download/v${APALACHE_VERSION}/sha256sum.txt /tmp/sha256sum.txt
RUN grep -qx "${APALACHE_SHA256}  apalache-${APALACHE_VERSION}.tgz" /tmp/sha256sum.txt \
    && echo "${APALACHE_SHA256}  /tmp/apalache.tgz" | sha256sum -c - \
    && mkdir -p ${QUINT_HOME}/apalache-dist-${APALACHE_VERSION}/apalache \
    && tar -xzf /tmp/apalache.tgz --strip-components=1 -C ${QUINT_HOME}/apalache-dist-${APALACHE_VERSION}/apalache \
    && rm /tmp/apalache.tgz /tmp/sha256sum.txt \
    && test -f ${QUINT_HOME}/apalache-dist-${APALACHE_VERSION}/apalache/lib/apalache.jar
