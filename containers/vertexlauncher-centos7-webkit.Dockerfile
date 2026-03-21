ARG BASE_IMAGE=docker.io/library/centos:7
FROM ${BASE_IMAGE}

RUN repo_prefix="" \
    && if [ "$(uname -m)" = "aarch64" ]; then repo_prefix="altarch/"; fi \
    && rm -f /etc/yum.repos.d/*.repo \
    && cat >/etc/yum.repos.d/CentOS-Vault.repo <<EOF
[base]
name=CentOS-7 - Base
baseurl=http://vault.centos.org/${repo_prefix}7.9.2009/os/\$basearch/
gpgcheck=0
enabled=1
[updates]
name=CentOS-7 - Updates
baseurl=http://vault.centos.org/${repo_prefix}7.9.2009/updates/\$basearch/
gpgcheck=0
enabled=1
[extras]
name=CentOS-7 - Extras
baseurl=http://vault.centos.org/${repo_prefix}7.9.2009/extras/\$basearch/
gpgcheck=0
enabled=1
EOF

RUN yum -y install \
      ca-certificates \
      curl \
      gcc \
      gcc-c++ \
      make \
      pkgconfig \
      patchelf \
      file \
      desktop-file-utils \
      glib2-devel \
      gtk3-devel \
      gdk-pixbuf2-devel \
      pango-devel \
      atk-devel \
      cairo-devel \
      dbus-devel \
      libsoup-devel \
      webkitgtk4-devel \
      webkitgtk4-jsc-devel \
      binutils >/dev/null \
    && yum clean all \
    && rm -rf /var/cache/yum
