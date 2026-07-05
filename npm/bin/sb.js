#!/usr/bin/env node
"use strict";

require("../lib/run")
  .run("sb", process.argv.slice(2))
  .then((code) => process.exit(code));
