"""Python packaging entrypoint for the ``tree_sitter_talkbank`` extension."""

from os import path
from sysconfig import get_config_var

from setuptools import Extension, find_packages, setup
from setuptools.command.build import build
from setuptools.command.build_ext import build_ext
from setuptools.command.egg_info import egg_info
from wheel.bdist_wheel import bdist_wheel


class Build(build):
    """Include query files in the wheel/install tree."""

    def run(self):
        """Copy query assets into the build tree before standard build steps."""
        if path.isdir("queries"):
            dest = path.join(self.build_lib, "tree_sitter_talkbank", "queries")
            self.copy_tree("queries", dest)
        super().run()


class BuildExt(build_ext):
    """Configure C extension build flags for each compiler target."""

    def build_extension(self, ext: Extension):
        """Compile one extension with platform flags and optional generated scanner."""
        if self.compiler.compiler_type != "msvc":
            ext.extra_compile_args = ["-std=c11", "-fvisibility=hidden"]
        else:
            ext.extra_compile_args = ["/std:c11", "/utf-8"]
        if path.exists("src/scanner.c"):
            ext.sources.append("src/scanner.c")
        if ext.py_limited_api:
            ext.define_macros.append(("Py_LIMITED_API", "0x030A0000"))
        super().build_extension(ext)


class BdistWheel(bdist_wheel):
    """Emit abi3 wheels so one build supports multiple Python versions."""

    def get_tag(self):
        """Force ``cp310-abi3`` tags so wheels remain usable on newer CPython versions."""
        python, abi, platform = super().get_tag()
        if python.startswith("cp"):
            python, abi = "cp310", "abi3"
        return python, abi, platform


class EggInfo(egg_info):
    """Ensure grammar query assets and headers are present in sdist metadata."""

    def find_sources(self):
        """Add non-Python grammar assets that setuptools would otherwise omit."""
        super().find_sources()
        self.filelist.recursive_include("queries", "*.scm")
        self.filelist.include("src/tree_sitter/*.h")


setup(
    packages=find_packages("bindings/python"),
    package_dir={"": "bindings/python"},
    package_data={
        "tree_sitter_talkbank": ["*.pyi", "py.typed"],
        "tree_sitter_talkbank.queries": ["*.scm"],
    },
    ext_package="tree_sitter_talkbank",
    ext_modules=[
        Extension(
            name="_binding",
            sources=[
                "bindings/python/tree_sitter_talkbank/binding.c",
                "src/parser.c",
            ],
            define_macros=[
                ("PY_SSIZE_T_CLEAN", None),
                ("TREE_SITTER_HIDE_SYMBOLS", None),
            ],
            include_dirs=["src"],
            py_limited_api=not get_config_var("Py_GIL_DISABLED"),
        )
    ],
    cmdclass={
        "build": Build,
        "build_ext": BuildExt,
        "bdist_wheel": BdistWheel,
        "egg_info": EggInfo,
    },
    zip_safe=False
)
