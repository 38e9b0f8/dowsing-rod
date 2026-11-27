import groovy.json.JsonSlurper

def log = new File(basedir, 'build.log').text
def report = { path -> new JsonSlurper().parse(new File(basedir, path)) }
def count = { text, needle -> text.split(java.util.regex.Pattern.quote(needle), -1).length - 1 }

assert log.contains('dowsing-rod check failed')
assert log.contains('exceeds maxClustersFound=0')
assert log.contains('StripeGateway.java')
assert report('target/dowsing-rod.json').clusters.size() > 0
true
