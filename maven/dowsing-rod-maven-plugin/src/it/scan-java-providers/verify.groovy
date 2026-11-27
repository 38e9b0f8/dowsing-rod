import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

def json = report('target/dowsing-rod.json')
assert json.statistics.files_scanned == 3
assert json.functions.every { it.language == 'java' }
def charge = json.clusters.find { c -> c.function_indices.collect { json.functions[it].function_name }.every { it == 'charge' } }
assert charge != null : 'charge methods should cluster'
assert charge.function_indices.size() == 3
assert log.contains('dowsing-rod report written to')
assert !log.contains('"schema_version"') : 'JSON goes to the file, not the log'
true
