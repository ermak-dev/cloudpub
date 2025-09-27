import cloudpub_python_sdk

project = 'CloudPub Python SDK'

project = 'cloudpub_python_sdk'
copyright = '2024-2025, CloudPub'
author = 'CloudPub'

extensions = [
    'sphinx.ext.autodoc',
    'sphinx.ext.autosummary',
    'sphinx.ext.intersphinx',
    'sphinx.ext.todo',
    'sphinx.ext.inheritance_diagram',
    'sphinx.ext.autosectionlabel',
    'sphinx.ext.napoleon',
    'sphinx_rtd_theme',
]

html_baseurl = '/docs/python-sdk/'

autosummary_generate = True
autosummary_imported_members = True

templates_path = ['_templates']
exclude_patterns = []

# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

html_theme = 'sphinx_rtd_theme'
html_static_path = ['_static']
