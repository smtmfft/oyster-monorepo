GO=go
GOVER=$(shell go version)
GOBUILD=$(GO) build
BINDIR=build
BINCLI=epcp
INSTALLLOC=/usr/local/bin/$(BINCLI)
RELEASE=$(shell git describe --tags --abbrev=0)
BUILDCOMMIT=$(shell git rev-parse HEAD | cut -c 1-7)
LATESTTAG=$(shell git describe --tags --abbrev=0)
BUILDLINE=$(shell git rev-parse --abbrev-ref HEAD)
CURRENTTIME=$(shell date -u '+%d-%m-%Y %H:%M:%S')
BUILDER=$(shell uname -n)

build:
	$(GOBUILD) -ldflags="\
	-X 'github.com/marlinprotocol/EnclaveProviderControlPlane/version.ApplicationVersion=$(LATESTTAG)' \
	-X 'github.com/marlinprotocol/EnclaveProviderControlPlane/version.buildCommit=$(BUILDLINE)@$(BUILDCOMMIT)' \
	-X 'github.com/marlinprotocol/EnclaveProviderControlPlane/version.buildTime=$(CURRENTTIME)' \
	-X 'github.com/marlinprotocol/EnclaveProviderControlPlane/version.builder=$(BUILDER)' \
	-X 'github.com/marlinprotocol/EnclaveProviderControlPlane/version.gover=$(GOVER)' \
	-linkmode=external" \
	-o $(BINDIR)/$(BINCLI)
clean:
	rm -rf $(BINDIR)/*

install:
	cp $(BINDIR)/$(BINCLI) $(INSTALLLOC)

uninstall:
	rm $(INSTALLLOC)