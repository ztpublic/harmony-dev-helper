plugins {
  kotlin("jvm") version "2.1.10"
  id("org.jetbrains.intellij") version "1.17.4"
}

group = "dev.harmony"
version = "0.1.0"

repositories {
  mavenCentral()
}

dependencies {
  implementation("org.json:json:20240303")
}

intellij {
  version.set("2024.3")
  type.set("IC")
}

tasks {
  patchPluginXml {
    sinceBuild.set("243")
    untilBuild.set("251.*")
  }

  withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
    kotlinOptions.jvmTarget = "17"
  }
}
