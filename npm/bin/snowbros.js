#!/usr/bin/env node
"use strict";

require("../lib/run")
  .run("snowbros", process.argv.slice(2))
  .then((code) => process.exit(code));
