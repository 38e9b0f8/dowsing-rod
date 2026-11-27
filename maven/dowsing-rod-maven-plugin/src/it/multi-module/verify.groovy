import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

def json = report('target/dowsing-rod.json')
def files = json.functions*.file
assert files.any { it.contains('billing') } && files.any { it.contains('invoicing') }
assert json.clusters.any { c -> c.function_indices.collect { json.functions[it].file }.toSet().size() == 2 } : 'duplication across modules should cluster'
assert count(log, 'dowsing-rod report written to') == 1
assert log.contains('runs once, at the reactor root')
assert !new File(basedir, 'billing/target/dowsing-rod.json').exists()
true
